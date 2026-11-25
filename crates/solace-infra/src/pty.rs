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
    output: Arc<Mutex<Output>>,
    paused: Arc<(Mutex<bool>, Condvar)>,
    screen: Arc<Mutex<Screen>>,
    activity: Arc<Mutex<Activity>>,
    pid: Option<u32>,
}

impl Pty {
    pub fn spawn(
        cwd: &Path,
        env: &[(&str, String)],
        cols: u16,
        rows: u16,
        output: Output,
        on_exit: impl FnOnce(Option<u32>) + Send + 'static,
    ) -> Result<Pty, PtyError> {
        let pair = native_pty_system().openpty(size(cols, rows))?;

        let shell = default_shell();
        let mut cmd = CommandBuilder::new(&shell);
        if is_powershell(&shell) {
            cmd.arg("-NoLogo");
        }
        cmd.cwd(cwd);
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
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (key, value) in env {
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
                {
                    let mut screen = lock_screen(&mirror);
                    if panic::catch_unwind(AssertUnwindSafe(|| screen.process(&pending))).is_err() {
                        let (rows, cols) = screen.screen().size();
                        *screen =
                            Screen::new_with_callbacks(rows, cols, 0, TitleTracker::default());
                    }
                }
                let now = Instant::now();
                let mut activity = activity_log.lock().unwrap();
                if now.duration_since(activity.last) >= QUIET {
                    activity.burst_start = now;
                }
                activity.last = now;
                drop(activity);
                sink(pending);
            }
            // The waiter reports before it closes the pty, which is what ended the output.
            let code = status_rx.recv().ok().flatten();
            if code != Some(0) {
                let code = code.map_or("unknown".to_string(), |c| (c as i32).to_string());
                let note = format!("\r\n\x1b[2m[process exited with code {code}]\x1b[0m\r\n");
                (sink.lock().unwrap())(note.into_bytes());
            }
            on_exit(code);
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
        output(
            lock_screen(&self.screen)
                .screen()
                .state_formatted()
                .into_bytes(),
        );
        *current = output;
    }

    pub fn snapshot(&self) -> Snapshot {
        let screen = lock_screen(&self.screen);
        let (_, cols) = screen.screen().size();
        Snapshot {
            title: screen.callbacks().title.clone(),
            rows: screen.screen().rows(0, cols.get()).collect(),
        }
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

fn is_powershell(shell: &Path) -> bool {
    shell
        .file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("pwsh") || s.eq_ignore_ascii_case("powershell"))
}

/// Windows keeps per-drive cwd entries like `=C:`, so a leading `=` is valid.
fn is_valid_env(key: &str, value: &str) -> bool {
    !key.is_empty()
        && !key.chars().skip(1).any(|c| c == '=')
        && !key.contains('\0')
        && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn spawned_shell_output_reaches_the_sink_and_exit_is_reported() {
        let (tx, rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let pty = Pty::spawn(
            &std::env::temp_dir(),
            &[("SOLACE_TEST", "marker-42".into())],
            80,
            24,
            Box::new(move |bytes| {
                let _ = tx.send(bytes);
            }),
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
        pty.write(b"\x1b[1;1R".to_vec());
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
