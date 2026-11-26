use std::error::Error;
use std::io::{Read, Write};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use std::num::NonZeroU16;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// A silence this long ends a burst of output.
const QUIET: Duration = Duration::from_millis(1500);
/// Output is coalesced for up to ~one frame, or this many bytes, per message to the UI.
const FLUSH_AFTER: Duration = Duration::from_millis(4);
const FLUSH_BYTES: usize = 64 * 1024;

pub type Output = Box<dyn FnMut(Vec<u8>) + Send>;
pub type PtyError = Box<dyn Error + Send + Sync>;

/// A headless copy of what the terminal shows, kept in Rust so agents' state can be read and a
/// reloaded UI can be repainted without asking the frontend.
type Screen = vt100::Parser<TitleTracker>;

#[derive(Default)]
struct TitleTracker {
    title: String,
}

impl vt100::Callbacks for TitleTracker {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }
}

/// The mirror must never take the app down: a parser bug only costs that pane's mirror.
fn lock_screen(screen: &Mutex<Screen>) -> MutexGuard<'_, Screen> {
    screen.lock().unwrap_or_else(PoisonError::into_inner)
}

/// When output started and when it last arrived, to tell a busy program from a one-off redraw.
#[derive(Clone, Copy)]
struct Activity {
    burst_start: Instant,
    last: Instant,
}

/// The visible screen as text.
pub struct Snapshot {
    pub title: String,
    pub rows: Vec<String>,
}

/// A shell running in a pseudo terminal.
///
/// Threads per pty: reader (blocks on the OS pipe, honours pause), flusher (coalesces chunks and
/// calls `output`, then `on_exit` with the exit code once output ends), writer (so input never
/// blocks a caller),
/// and waiter (closes the pty when the shell exits, which is what ends the reader on Windows).
pub struct Pty {
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    input: mpsc::Sender<Vec<u8>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    output: Arc<Mutex<Option<Output>>>,
    paused: Arc<(Mutex<bool>, Condvar)>,
    screen: Arc<Mutex<Screen>>,
    activity: Arc<Mutex<Activity>>,
    pid: Option<u32>,
}

/// What to start and where.
pub struct Launch<'a> {
    pub cwd: &'a Path,
    pub env: &'a [(&'a str, String)],
    /// Prepended to PATH (where the `solace` CLI lives).
    pub bin_dir: Option<&'a Path>,
    /// The shell the user picked; the platform default when `None` or no longer installed.
    pub shell: Option<&'a Path>,
}

/// A shell the user can pick for new terminals.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Shell {
    pub name: &'static str,
    pub path: PathBuf,
}

/// Shells installed on this machine, the default first.
pub fn available_shells() -> Vec<Shell> {
    let mut shells = Vec::new();
    let mut add = |name, path: Option<PathBuf>| {
        if let Some(path) = path.filter(|p| p.is_file())
            && !shells.iter().any(|s: &Shell| s.path == path)
        {
            shells.push(Shell { name, path });
        }
    };
    if cfg!(windows) {
        add("PowerShell 7", which::which("pwsh").ok());
        add("Windows PowerShell", which::which("powershell").ok());
        add("Command Prompt", which::which("cmd").ok());
        // Git's own bash, not System32's bash.exe (which starts WSL).
        // git.exe lives in Git\cmd or Git\mingw64\bin depending on how PATH was set up. The
        // install root is the folder with git-bash.exe, so a stray ~\bin\bash.exe isn't taken
        // for it.
        let git_bash = which::which("git").ok().and_then(|git| {
            git.ancestors()
                .skip(1)
                .take(4)
                .find(|dir| dir.join("git-bash.exe").is_file())
                .map(|root| root.join("bin").join("bash.exe"))
        });
        add("Git Bash", git_bash);
        add("WSL", which::which("wsl").ok());
    } else {
        add("Default", std::env::var_os("SHELL").map(PathBuf::from));
        for name in ["bash", "zsh", "fish"] {
            add(name, which::which(name).ok());
        }
    }
    shells
}

impl Pty {
    pub fn spawn(
        launch: &Launch,
        cols: u16,
        rows: u16,
        // None: nobody is watching yet (a pane an agent opened); see the flusher.
        output: Option<Output>,
        on_exit: impl FnOnce(Option<u32>) + Send + 'static,
    ) -> Result<Pty, PtyError> {
        let pair = native_pty_system().openpty(size(cols, rows))?;

        let shell = launch
            .shell
            .filter(|s| s.is_file())
            .map(Path::to_path_buf)
            .unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(&shell);
        cmd.args(shell_args(&shell));
        cmd.cwd(launch.cwd);
        // The base env is re-read from the registry on Windows (so newly installed tools show up);
        // malformed entries there make CreateProcessW fail with error 87.
        let bad: Vec<String> = cmd
            .iter_full_env_as_str()
            .filter(|(key, value)| !is_valid_env(key, value))
            .map(|(key, _)| key.to_string())
            .collect();
        for key in bad {
            cmd.env_remove(key);
        }
        if let Some(dir) = launch.bin_dir {
            // Puts the `solace` CLI on PATH for shells and the agents they start.
            let path = cmd.get_env("PATH").map(std::env::split_paths);
            let paths = std::iter::once(dir.to_path_buf()).chain(path.into_iter().flatten());
            if let Ok(joined) = std::env::join_paths(paths) {
                cmd.env("PATH", joined);
            }
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in launch.env {
            cmd.env(key, value);
        }

        // Children inherit "ignore Ctrl+C" from us, and launchers (task runners, IDEs) often set it;
        // without this reset Ctrl+C in a pane never interrupts anything.
        #[cfg(windows)]
        // SAFETY: a null handler with FALSE only clears the process-wide ignore flag.
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
        }
        let mut child = pair.slave.spawn_command(cmd)?;
        // Only the master may keep the pty open, so the output ends when the shell does.
        drop(pair.slave);
        let killer = child.clone_killer();
        let pid = child.process_id();
        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let output = Arc::new(Mutex::new(output));
        let paused = Arc::new((Mutex::new(false), Condvar::new()));
        let screen = Arc::new(Mutex::new(Screen::new_with_callbacks(
            nonzero(rows),
            nonzero(cols),
            0,
            TitleTracker::default(),
        )));
        let started = Instant::now() - QUIET;
        let activity = Arc::new(Mutex::new(Activity {
            burst_start: started,
            last: started,
        }));

        let (input, input_rx) = mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            for bytes in input_rx {
                if writer
                    .write_all(&bytes)
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        });

        let (chunks, chunks_rx) = mpsc::channel::<Vec<u8>>();
        let flow = Arc::clone(&paused);
        thread::spawn(move || {
            let mut buf = vec![0u8; FLUSH_BYTES];
            loop {
                let (lock, resumed) = &*flow;
                drop(
                    resumed
                        .wait_while(lock.lock().unwrap(), |paused| *paused)
                        .unwrap(),
                );
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if chunks.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let (status_tx, status_rx) = mpsc::channel::<Option<u32>>();
        let sink = Arc::clone(&output);
        let replies = input.clone();
        let mirror = Arc::clone(&screen);
        let activity_log = Arc::clone(&activity);
        thread::spawn(move || {
            while let Ok(mut pending) = chunks_rx.recv() {
                let deadline = Instant::now() + FLUSH_AFTER;
                while pending.len() < FLUSH_BYTES {
                    match chunks_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    {
                        Ok(more) => pending.extend_from_slice(&more),
                        // A disconnect is picked up by the outer recv after this flush.
                        Err(_) => break,
                    }
                }
                // Holding the sink while updating the mirror keeps a repaint (set_output) from
                // also receiving a chunk it already contains.
                let mut sink = sink.lock().unwrap();
                let cursor = {
                    let mut screen = lock_screen(&mirror);
                    if panic::catch_unwind(AssertUnwindSafe(|| screen.process(&pending))).is_err() {
                        let (rows, cols) = screen.screen().size();
                        *screen =
                            Screen::new_with_callbacks(rows, cols, 0, TitleTracker::default());
                    }
                    screen.screen().cursor_position()
                };
                let now = Instant::now();
                let mut activity = activity_log.lock().unwrap();
                if now.duration_since(activity.last) >= QUIET {
                    activity.burst_start = now;
                }
                activity.last = now;
                drop(activity);
                match sink.as_mut() {
                    Some(sink) => sink(pending),
                    // Unwatched, so answer ConPTY's startup queries as a terminal would: the shell
                    // waits for the cursor position, and newer ConPTY waits 3 s for the device
                    // attributes.
                    None => {
                        let asks = |query: &[u8]| pending.windows(query.len()).any(|w| w == query);
                        let mut reply = Vec::new();
                        if asks(b"\x1b[6n") {
                            let (row, col) = cursor;
                            reply.extend(format!("\x1b[{};{}R", row + 1, col + 1).into_bytes());
                        }
                        if asks(b"\x1b[c") || asks(b"\x1b[0c") {
                            reply.extend(b"\x1b[?1;2c"); // what xterm.js answers
                        }
                        if !reply.is_empty() {
                            let _ = replies.send(reply);
                        }
                    }
                }
            }
            // The waiter reports before it closes the pty, which is what ended the output.
            let code = status_rx.recv().ok().flatten();
            // First, so the pane counts as ended before the user can read the note and press Enter.
            on_exit(code);
            if code != Some(0) {
                let code = code.map_or("unknown".to_string(), |c| (c as i32).to_string());
                let note = format!(
                    "\r\n\x1b[2m[process exited with code {code} · press Enter to restart]\x1b[0m\r\n"
                );
                if let Some(sink) = sink.lock().unwrap().as_mut() {
                    sink(note.into_bytes());
                }
            }
        });

        let pty = Arc::clone(&master);
        let flow = Arc::clone(&paused);
        thread::spawn(move || {
            let code = child.wait().ok().map(|status| status.exit_code());
            let _ = status_tx.send(code);
            // Closing the pseudoconsole blocks until its output is drained, so the reader must be
            // running and the master lock released, or a resize would wait on us forever.
            *flow.0.lock().unwrap() = false;
            flow.1.notify_all();
            let master = pty.lock().unwrap().take();
            drop(master);
        });

        Ok(Pty {
            master,
            input,
            killer,
            output,
            paused,
            screen,
            activity,
            pid,
        })
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let _ = self.input.send(bytes);
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let mut screen = lock_screen(&self.screen);
        let _ = panic::catch_unwind(AssertUnwindSafe(|| {
            screen.set_size(nonzero(rows), nonzero(cols));
        }));
        drop(screen);
        if let Some(master) = &*self.master.lock().unwrap() {
            master.resize(size(cols, rows))?;
        }
        Ok(())
    }

    /// Redirects output, e.g. to a reloaded UI, repainting the current screen and restoring input
    /// modes (bracketed paste, mouse, cursor keys) there first.
    pub fn set_output(&self, mut output: Output) {
        let mut current = self.output.lock().unwrap();
        let screen = lock_screen(&self.screen);
        // The title too: it was set once, long before this new UI started listening.
        let title = screen.callbacks().title.replace(['\x07', '\x1b'], "");
        let repaint = format!("\x1b]0;{title}\x07{}", screen.screen().state_formatted());
        drop(screen);
        output(repaint.into_bytes());
        *current = Some(output);
    }

    pub fn snapshot(&self) -> Snapshot {
        let screen = lock_screen(&self.screen);
        let (_, cols) = screen.screen().size();
        Snapshot {
            title: screen.callbacks().title.clone(),
            rows: screen.screen().rows(0, cols.get()).collect(),
        }
    }

    /// Whether the program in the pane asked for bracketed paste.
    pub fn bracketed_paste(&self) -> bool {
        lock_screen(&self.screen).screen().bracketed_paste()
    }

    /// How long output has been streaming without a pause; `None` once it has gone quiet.
    pub fn busy_for(&self) -> Option<Duration> {
        let activity = *self.activity.lock().unwrap();
        (activity.last.elapsed() < QUIET).then(|| activity.burst_start.elapsed())
    }

    /// The shell's process id; agents run among its descendants.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Backpressure: while paused the OS pipe fills and the shell blocks on its own writes.
    pub fn set_paused(&self, paused: bool) {
        let (lock, resumed) = &*self.paused;
        *lock.lock().unwrap() = paused;
        resumed.notify_all();
    }

    pub fn kill(mut self) {
        // A paused reader would keep the pty from draining and closing.
        self.set_paused(false);
        let _ = self.killer.kill();
    }
}

fn nonzero(n: u16) -> NonZeroU16 {
    NonZeroU16::new(n).unwrap_or(NonZeroU16::MIN)
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn default_shell() -> PathBuf {
    if cfg!(windows) {
        which::which("pwsh")
            .or_else(|_| which::which("powershell"))
            .unwrap_or_else(|_| PathBuf::from("cmd.exe"))
    } else {
        std::env::var_os("SHELL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/bin/sh"))
    }
}

fn shell_args(shell: &Path) -> &'static [&'static str] {
    let stem = shell
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match stem.as_str() {
        "pwsh" | "powershell" => &["-NoLogo"],
        // A login shell reads the user's profile, as Git Bash's own window does.
        "bash" => &["--login", "-i"],
        // Started from the Dock or a launcher, the app's own PATH is minimal; the login profile
        // brings the user's (Homebrew, ~/.local/bin, version managers).
        "zsh" | "fish" => &["-l"],
        _ => &[],
    }
}

/// Windows keeps per-drive cwd entries like `=C:`, so a leading `=` is valid.
fn is_valid_env(key: &str, value: &str) -> bool {
    !key.is_empty()
        && !key.chars().skip(1).any(|c| c == '=')
        && !key.contains('\0')
        && !value.contains('\0')
}

/// Makes ptys use the ConPTY in `dir` (Solace's pinned copy, newer than Windows') and stops DLLs
/// from being found through PATH or the working directory. Call before the first pty.
/// `SOLACE_WINDOWS_CONPTY=system` keeps the ConPTY built into Windows.
#[cfg(windows)]
pub fn use_bundled_conpty(dir: &Path) -> Result<(), String> {
    use windows_sys::Win32::System::LibraryLoader::{
        LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LoadLibraryExW,
        SetDefaultDllDirectories,
    };
    // portable-pty loads "conpty.dll" by bare name, which would otherwise search PATH.
    // SAFETY: takes only flags; it narrows where this process looks for DLLs from now on.
    if unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) } == 0 {
        return Err(format!(
            "SetDefaultDllDirectories: {}",
            std::io::Error::last_os_error()
        ));
    }
    if std::env::var("SOLACE_WINDOWS_CONPTY").as_deref() == Ok("system") {
        return Ok(());
    }
    let dll = dir.join("conpty.dll");
    // Tauri's paths are verbatim; turn `\\?\C:\..` and `\\?\UNC\srv\..` back into normal paths.
    let path = dll.to_string_lossy();
    let path = match path.strip_prefix(r"\\?\") {
        Some(rest) => match rest.strip_prefix(r"UNC\") {
            Some(share) => format!(r"\\{share}"),
            None => rest.to_string(),
        },
        None => path.to_string(),
    };
    let wide = widestring::U16CString::from_str(&path).map_err(|e| e.to_string())?;
    // Once loaded, the bare-name load in portable-pty resolves to this module.
    // SAFETY: `wide` is a NUL-terminated absolute path that outlives the call; no file handle.
    let module = unsafe {
        LoadLibraryExW(
            wide.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    };
    if module.is_null() {
        return Err(format!("{path}: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_installed_shells_with_the_right_arguments() {
        let shells = available_shells();
        assert!(!shells.is_empty());
        assert!(shells.iter().all(|s| s.path.is_file()));
        assert_eq!(shell_args(Path::new("C:/x/pwsh.exe")), ["-NoLogo"]);
        assert_eq!(
            shell_args(Path::new("C:/Git/bin/bash.exe")),
            ["--login", "-i"]
        );
        assert!(shell_args(Path::new("C:/Windows/System32/cmd.exe")).is_empty());
    }

    #[test]
    fn rejects_env_entries_that_break_process_creation() {
        assert!(is_valid_env("PATH", "C:\\bin"));
        assert!(is_valid_env("=C:", "C:\\Users"));
        assert!(is_valid_env("EMPTY", ""));
        assert!(!is_valid_env("", "x"));
        assert!(!is_valid_env("A=B", "x"));
        assert!(!is_valid_env("A", "x\0y"));
    }

    #[test]
    fn unwatched_shell_still_starts_and_runs_commands() {
        // No sink: Solace itself must answer ConPTY's startup cursor query.
        let cwd = std::env::temp_dir();
        let launch = Launch {
            cwd: &cwd,
            env: &[],
            bin_dir: None,
            shell: None,
        };
        let pty = Pty::spawn(&launch, 80, 24, None, |_| {}).unwrap();
        pty.write(b"echo headless-ok\r".to_vec());
        let deadline = Instant::now() + Duration::from_secs(20);
        while !pty
            .snapshot()
            .rows
            .iter()
            .any(|r| r.trim() == "headless-ok")
        {
            assert!(
                Instant::now() < deadline,
                "shell never ran: {:?}",
                pty.snapshot().rows
            );
            thread::sleep(Duration::from_millis(100));
        }
        pty.kill();
    }

    #[test]
    fn spawned_shell_output_reaches_the_sink_and_exit_is_reported() {
        let (tx, rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let cwd = std::env::temp_dir();
        let env = [("SOLACE_TEST", "marker-42".to_string())];
        // Plain sh on Unix: a login bash's prompt (Ubuntu's) resets the title after each command.
        let sh = Path::new("/bin/sh");
        let launch = Launch {
            cwd: &cwd,
            env: &env,
            bin_dir: None,
            shell: (!cfg!(windows)).then_some(sh),
        };
        let pty = Pty::spawn(
            &launch,
            80,
            24,
            Some(Box::new(move |bytes| {
                let _ = tx.send(bytes);
            })),
            move |code| {
                let _ = exit_tx.send(code);
            },
        )
        .unwrap();

        let (echo, exit) = if cfg!(windows) {
            (
                "$host.UI.RawUI.WindowTitle = 'solace-title'; echo $env:SOLACE_TEST\r",
                "exit\r",
            )
        } else {
            (
                "printf '\\033]0;solace-title\\007'; echo $SOLACE_TEST\n",
                "exit\n",
            )
        };
        // ConPTY asks for the cursor position (DSR) at startup and waits; xterm answers in the app.
        // A Unix shell asked nothing, and would take the reply for typed text.
        if cfg!(windows) {
            pty.write(b"\x1b[1;1R".to_vec());
        }
        pty.write(echo.as_bytes().to_vec());

        let mut seen = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while !String::from_utf8_lossy(&seen).contains("marker-42") {
            let left = deadline.saturating_duration_since(Instant::now());
            let chunk = rx
                .recv_timeout(left)
                .unwrap_or_else(|_| panic!("no marker in {:?}", String::from_utf8_lossy(&seen)));
            seen.extend(chunk);
        }
        let snapshot = pty.snapshot();
        assert_eq!(snapshot.title, "solace-title");
        assert!(snapshot.rows.iter().any(|row| row.trim() == "marker-42"));
        assert!(pty.busy_for().is_some());

        pty.write(exit.as_bytes().to_vec());
        let code = exit_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("exit reported");
        assert_eq!(code, Some(0));
    }
}
