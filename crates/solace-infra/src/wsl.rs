//! Agents running inside WSL, which the Windows process table can't see. WSLENV carries each
//! pane's SOLACE_PANE_ID and SOLACE_PIPE into Linux, so the distro's /proc tells which
//! processes belong to which pane of this Solace.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use solace_core::agents::{self, AgentDef};

/// Every second, prints `pipe TAB pane TAB pid TAB ppid TAB name TAB args` (args split by 0x1F)
/// for each process that has SOLACE_PANE_ID, then a line with a single dot. POSIX sh and busybox.
const SCAN: &str = r#"while :; do
for f in $(grep -lsa 'SOLACE_PANE_ID=' /proc/[0-9]*/environ); do
d=${f%/environ}
env=$(tr '\0' '\n' < "$f")
pane=$(printf '%s\n' "$env" | sed -n 's/^SOLACE_PANE_ID=//p')
pipe=$(printf '%s\n' "$env" | sed -n 's/^SOLACE_PIPE=//p')
read -r s < "$d/stat"; s=${s##*) }; set -- $s
comm=; read -r comm < "$d/comm"
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$pipe" "$pane" "${d#/proc/}" "$2" "$comm" "$(tr '\0' '\037' < "$d/cmdline")"
done 2>/dev/null
echo .
sleep 1
done
"#;

/// After a distro's scanner dies (the distro stopped), wait before starting another, so a
/// session that is itself ending can't make Solace boot the distro again.
const RESTART_AFTER: Duration = Duration::from_secs(5);

/// The distro a `wsl.exe` command line opens a shell or command in (`None`: the default one),
/// or `None` when it is a management command such as `wsl --shutdown` or `wsl -l`.
pub fn session(args: &[String]) -> Option<Option<String>> {
    let mut distro = None;
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-d" | "--distribution" => distro = Some(rest.next()?.to_lowercase()),
            "-u" | "--user" | "--cd" | "--shell-type" => {
                rest.next()?;
            }
            "~" | "--system" => {}
            "-e" | "--exec" | "--" => break,
            flag if flag.starts_with('-') => return None,
            _ => break,
        }
    }
    Some(distro)
}

/// One long-lived scanner per distro in use, so detection costs no process start per second
/// and never blocks the caller.
#[derive(Default)]
pub struct Wsl {
    scanners: HashMap<Option<String>, Scanner>,
}

struct Scanner {
    child: Child,
    /// The last complete listing.
    latest: Arc<Mutex<String>>,
    died: Arc<Mutex<Option<Instant>>>,
}

impl Drop for Scanner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Scanner {
    fn start(distro: Option<&str>) -> Option<Scanner> {
        let mut command = Command::new("wsl.exe");
        if let Some(distro) = distro {
            command.args(["-d", distro]);
        }
        // Root reads every process's environment; only rows naming this Solace's pipe are used.
        command
            .args(["-u", "root", "-e", "sh", "-s"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().ok()?;
        // The script goes in on stdin, so no quoting has to survive wsl.exe's argument handling.
        let mut stdin = child.stdin.take()?;
        stdin.write_all(SCAN.as_bytes()).ok()?;
        drop(stdin);
        let stdout = child.stdout.take()?;
        let latest = Arc::new(Mutex::new(String::new()));
        let died = Arc::new(Mutex::new(None));
        let (out, dead) = (latest.clone(), died.clone());
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let (mut block, mut line) = (String::new(), Vec::new());
            loop {
                line.clear();
                match reader.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                // Lossy: one odd byte in a command line must not hide the whole distro.
                let text = String::from_utf8_lossy(&line);
                if text.trim_end() == "." {
                    *out.lock().unwrap() = std::mem::take(&mut block);
                } else {
                    block.push_str(&text);
                }
            }
            *dead.lock().unwrap() = Some(Instant::now());
        });
        Some(Scanner {
            child,
            latest,
            died,
        })
    }
}

impl Wsl {
    /// The agent closest to each pane's shell inside WSL. `sessions` pairs each pane with a
    /// distro it has a WSL session in; `pipe` is this Solace's SOLACE_PIPE.
    pub fn agents(
        &mut self,
        sessions: &[(u32, Option<String>)],
        pipe: &str,
    ) -> HashMap<u32, &'static AgentDef> {
        let wanted: HashSet<&Option<String>> = sessions.iter().map(|(_, d)| d).collect();
        self.scanners.retain(|distro, scanner| {
            wanted.contains(distro)
                && scanner
                    .died
                    .lock()
                    .unwrap()
                    .is_none_or(|at| at.elapsed() < RESTART_AFTER)
        });
        for distro in wanted {
            if !self.scanners.contains_key(distro)
                && let Some(scanner) = Scanner::start(distro.as_deref())
            {
                self.scanners.insert(distro.clone(), scanner);
            }
        }
        let mut found = HashMap::new();
        for (distro, scanner) in &self.scanners {
            let listing = scanner.latest.lock().unwrap().clone();
            let mine: HashSet<u32> = sessions
                .iter()
                .filter(|(_, d)| d == distro)
                .map(|(pane, _)| *pane)
                .collect();
            let processes: Vec<LinuxProcess> = parse(&listing)
                .into_iter()
                .filter(|p| p.pipe == pipe && mine.contains(&p.pane))
                .collect();
            found.extend(closest_agents(&processes));
        }
        found
    }
}

struct LinuxProcess {
    pipe: String,
    pane: u32,
    pid: u32,
    ppid: u32,
    name: String,
    args: Vec<String>,
}

fn parse(out: &str) -> Vec<LinuxProcess> {
    out.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(6, '\t');
            let pipe = fields.next()?.to_string();
            let mut number = || fields.next()?.trim().parse().ok();
            let (pane, pid, ppid) = (number()?, number()?, number()?);
            let name = fields.next()?.to_string();
            let args = fields
                .next()?
                .split('\u{1f}')
                .filter(|a| !a.is_empty())
                .map(str::to_string)
                .collect();
            Some(LinuxProcess {
                pipe,
                pane,
                pid,
                ppid,
                name,
                args,
            })
        })
        .collect()
}

/// By process name, else by command: npm installs agents as scripts that run as
/// `node /usr/local/bin/gemini`, with no node_modules path in the command line.
fn identify(process: &LinuxProcess) -> Option<&'static AgentDef> {
    let base = |arg: &String| arg.rsplit('/').next().unwrap_or(arg).to_string();
    let program = process.args.first().map(base);
    let interpreted = program.as_deref().is_some_and(|p| {
        ["node", "bun", "deno", "sh", "bash"].contains(&p) || p.starts_with("python")
    });
    let script = interpreted
        .then(|| process.args.iter().skip(1).find(|a| !a.starts_with('-')))
        .flatten()
        .map(base);
    [Some(process.name.clone()), program, script]
        .into_iter()
        .flatten()
        .find_map(|exe| agents::identify(&exe, &process.args))
}

/// Like the Windows scan: closest to the pane's shell wins, so an agent's helpers never do.
fn closest_agents(processes: &[LinuxProcess]) -> HashMap<u32, &'static AgentDef> {
    let parents: HashMap<(u32, u32), u32> = processes
        .iter()
        .map(|p| ((p.pane, p.pid), p.ppid))
        .collect();
    let mut best: HashMap<u32, (usize, &'static AgentDef)> = HashMap::new();
    for process in processes {
        let Some(agent) = identify(process) else {
            continue;
        };
        // Generations up to the pane's first process (its parent is outside the pane).
        let mut depth = 0;
        let mut pid = process.pid;
        while let Some(&parent) = parents.get(&(process.pane, pid)) {
            if depth > 64 || !parents.contains_key(&(process.pane, parent)) {
                break;
            }
            depth += 1;
            pid = parent;
        }
        let entry = best.entry(process.pane).or_insert((depth, agent));
        if depth < entry.0 {
            *entry = (depth, agent);
        }
    }
    best.into_iter()
        .map(|(pane, (_, agent))| (pane, agent))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str) -> Vec<LinuxProcess> {
        parse(&text.replace('|', "\t").replace('^', "\u{1f}"))
    }

    #[test]
    fn finds_the_agent_closest_to_each_panes_shell() {
        let found = closest_agents(&rows(
            "p|3|10|1|bash|-bash\n\
             p|3|20|10|node|node^/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js\n\
             p|3|30|20|codex|codex^mcp\n\
             p|4|40|1|bash|-bash\n\
             p|5|50|1|sh|sh\n\
             p|5|51|50|codex|codex\n\
             p|6|60|1|node|node^--no-warnings^/usr/local/bin/gemini\n\
             p|8|80|1|vim|vim^claude\n\
             p|9|90|1||/bin/sh^/tmp/claude\n\
             garbage line\n",
        ));
        assert_eq!(
            found[&3].id, "claude",
            "the helper under claude doesn't win"
        );
        assert_eq!(found[&5].id, "codex");
        assert!(!found.contains_key(&4));
        assert_eq!(
            found[&6].id, "gemini",
            "an npm script run through its symlink"
        );
        assert!(
            !found.contains_key(&8),
            "editing a file named claude is not claude"
        );
        assert_eq!(
            found[&9].id, "claude",
            "a script, even without /proc/<pid>/comm"
        );
    }

    #[test]
    fn tells_sessions_from_management_commands() {
        let args = |s: &str| s.split(' ').map(str::to_string).collect::<Vec<_>>();
        assert_eq!(session(&args("wsl.exe")), Some(None));
        assert_eq!(
            session(&args("wsl.exe ~ -d Ubuntu")),
            Some(Some("ubuntu".into()))
        );
        assert_eq!(
            session(&args("wsl.exe --distribution Debian -u root --cd /tmp")),
            Some(Some("debian".into()))
        );
        assert_eq!(session(&args("wsl.exe -e ls -d x")), Some(None));
        assert_eq!(session(&args("wsl.exe ls -la")), Some(None));
        for command in [
            "wsl.exe --shutdown",
            "wsl.exe -t Ubuntu",
            "wsl.exe -l -v",
            "wsl.exe --install -d Ubuntu",
        ] {
            assert_eq!(session(&args(command)), None, "{command}");
        }
    }

    /// Needs a WSL distro: `SOLACE_WSL_TEST_DISTRO=<name> cargo test -p solace-infra wsl -- --ignored`.
    #[test]
    #[ignore]
    fn sees_a_fake_claude_inside_a_real_distro() {
        let distro = std::env::var("SOLACE_WSL_TEST_DISTRO")
            .unwrap()
            .to_lowercase();
        let mut agent = Command::new("wsl.exe")
            .args(["-d", &distro, "-e", "sh", "-c"])
            .arg("printf '#!/bin/sh\\nsleep 30\\n' > /tmp/claude; chmod +x /tmp/claude; exec /tmp/claude")
            .env("SOLACE_PANE_ID", "7")
            .env("SOLACE_PIPE", "test-pipe")
            .env("WSLENV", "SOLACE_PANE_ID:SOLACE_PIPE")
            .spawn()
            .unwrap();
        let mut wsl = Wsl::default();
        let sessions = [(7, Some(distro))];
        let deadline = Instant::now() + Duration::from_secs(15);
        let found = loop {
            let found = wsl.agents(&sessions, "test-pipe");
            if !found.is_empty() || Instant::now() > deadline {
                break found;
            }
            std::thread::sleep(Duration::from_millis(500));
        };
        let other_run = wsl.agents(&sessions, "another-pipe");
        let _ = agent.kill();
        let _ = agent.wait();
        assert_eq!(found.get(&7).map(|a| a.id.as_str()), Some("claude"));
        assert!(other_run.is_empty(), "another Solace's panes are not ours");
    }
}
