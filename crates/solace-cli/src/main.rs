use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use interprocess::local_socket::{Stream, prelude::*};
use solace_core::agents::hooks::{HookVerdict, claude_hook};
use solace_core::orchestration::{Request, Response};
use solace_core::tasks::TaskSpec;
use solace_core::terminals::{Direction, PaneId};

const SKILL: &str = include_str!("../../../skills/solace/SKILL.md");

/// Control the Solace workspace you are running in.
#[derive(Parser)]
#[command(name = "solace", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Panes: list, split, type into, read and close.
    #[command(subcommand)]
    Pane(PaneCommand),
    /// Agents: start, prompt, wait for and list.
    #[command(subcommand)]
    Agent(AgentCommand),
    /// Parallel tasks: start a batch of worker agents, wait for them, clean up.
    #[command(subcommand)]
    Tasks(TasksCommand),
    /// For a worker agent: report that its task is finished.
    #[command(subcommand)]
    Task(TaskCommand),
    /// Print the agent skill, or install it for Claude Code with `solace skill install`.
    Skill { action: Option<String> },
    /// Called by Claude Code's hooks with the event on stdin; reports the agent's exact state.
    #[command(hide = true)]
    Hook {
        /// Marks the hook as Solace's in Claude Code's settings.
        #[arg(long)]
        source: Option<String>,
    },
    /// Claude Code's status line command: reports the rate limits it receives on stdin.
    #[command(hide = true)]
    Statusline {
        /// Pass stdin through to stdout, for the user's own status line behind this one.
        #[arg(long)]
        tee: bool,
        #[arg(long)]
        source: Option<String>,
    },
}

#[derive(Subcommand)]
enum PaneCommand {
    List,
    Split {
        /// Pane to split; defaults to the one this command runs in.
        #[arg(long)]
        pane: Option<u32>,
        /// Split below instead of to the right.
        #[arg(long)]
        down: bool,
    },
    /// Type text into a pane, then press Enter.
    Send {
        target: String,
        text: String,
        #[arg(long)]
        no_enter: bool,
    },
    /// Print a pane's screen as text.
    Read {
        target: String,
        /// Only the last N non-empty lines.
        #[arg(long)]
        lines: Option<usize>,
    },
    Close {
        target: String,
    },
}

#[derive(Subcommand)]
enum AgentCommand {
    /// Split this pane and start an agent in it.
    Start {
        name: String,
        /// claude, codex, antigravity, qwen, kimi, opencode, copilot, cursor or amp.
        #[arg(long)]
        kind: String,
        #[arg(long)]
        down: bool,
        /// First task, sent once the agent is ready.
        #[arg(long)]
        prompt: Option<String>,
    },
    /// Send a prompt to an agent.
    Prompt {
        target: String,
        text: String,
        /// Wait until the agent stops working and print its screen.
        #[arg(long)]
        wait: bool,
        /// Seconds; the default stays under the 2-minute limit agents' shell tools usually have.
        #[arg(long, default_value_t = 110)]
        timeout: u64,
    },
    /// Wait until an agent is idle or needs the user.
    Wait {
        target: String,
        /// Seconds; the default stays under the 2-minute limit agents' shell tools usually have.
        #[arg(long, default_value_t = 110)]
        timeout: u64,
    },
    List,
}

#[derive(Subcommand)]
enum TasksCommand {
    /// Start one worker agent per task (a JSON array of {title, prompt, kind?, base?}, on stdin or in
    /// --file), each in its own git worktree and pane.
    Start {
        /// An agent kind, or "auto" for the one with the most room left in its usage limits.
        /// Default: the kind of the agent running this command.
        #[arg(long)]
        kind: Option<String>,
        /// Read the tasks from a UTF-8 file instead of stdin (PowerShell 5.1 mangles piped text).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Allow a folder that is not a git repository: the workers then edit it together.
        #[arg(long)]
        shared: bool,
    },
    /// Wait until every task of a batch is finished (default batch: your newest).
    Wait {
        batch: Option<String>,
        /// Seconds; the default stays under the 2-minute limit agents' shell tools usually have.
        #[arg(long, default_value_t = 110)]
        timeout: u64,
    },
    /// Show where the tasks of a batch stand, without waiting.
    List { batch: Option<String> },
    /// Close a batch's panes and remove its worktrees; branches are kept.
    Clean {
        batch: String,
        /// Remove worktrees even with uncommitted changes.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum TaskCommand {
    /// Report the result of your task (commit your work first).
    Done {
        #[arg(long, conflicts_with = "summary_file")]
        summary: Option<String>,
        /// Read the summary from a UTF-8 file: for long ones, or ones with quotes.
        #[arg(long)]
        summary_file: Option<PathBuf>,
        /// You could not finish; the summary says why.
        #[arg(long)]
        failed: bool,
        /// A review with nothing to fix: report without commits on the branch.
        #[arg(long)]
        no_changes: bool,
    },
}

/// The tasks for `tasks start`: from the file, else from piped stdin.
fn read_tasks(file: Option<PathBuf>) -> Result<Vec<TaskSpec>, String> {
    let mut bytes = Vec::new();
    match file {
        Some(path) => {
            bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        None => {
            if std::io::stdin().is_terminal() {
                return Err("give the tasks as a JSON array on stdin or with --file".into());
            }
            std::io::stdin()
                .take(4 * 1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
    }
    let text = String::from_utf8(bytes).map_err(|_| "the tasks must be UTF-8 text")?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| {
        format!(
            "the tasks must be a JSON array of {{\"title\", \"prompt\", \"kind\"?, \"base\"?}}: {e}"
        )
    })
}

fn task_env() -> Result<String, String> {
    std::env::var("SOLACE_TASK_ID")
        .ok()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "SOLACE_TASK_ID is not set: only a task's worker can run this".into())
}

fn caller_pane() -> Option<PaneId> {
    std::env::var("SOLACE_PANE_ID")
        .ok()?
        .parse()
        .ok()
        .map(PaneId)
}

fn direction(down: bool) -> Direction {
    if down {
        Direction::Column
    } else {
        Direction::Row
    }
}

fn request(command: Command) -> Result<Request, String> {
    Ok(match command {
        Command::Pane(PaneCommand::List) => Request::PaneList,
        Command::Pane(PaneCommand::Split { pane, down }) => Request::PaneSplit {
            pane: pane.map(PaneId).or_else(caller_pane),
            direction: direction(down),
        },
        Command::Pane(PaneCommand::Send {
            target,
            text,
            no_enter,
        }) => Request::PaneSend {
            target,
            text,
            enter: !no_enter,
        },
        Command::Pane(PaneCommand::Read { target, lines }) => Request::PaneRead { target, lines },
        Command::Pane(PaneCommand::Close { target }) => Request::PaneClose { target },
        Command::Agent(AgentCommand::Start {
            name,
            kind,
            down,
            prompt,
        }) => Request::AgentStart {
            name,
            kind,
            from: caller_pane(),
            direction: direction(down),
            prompt,
        },
        Command::Agent(AgentCommand::Prompt {
            target,
            text,
            wait,
            timeout,
        }) => Request::AgentPrompt {
            target,
            text,
            wait,
            timeout_secs: timeout,
        },
        Command::Agent(AgentCommand::Wait { target, timeout }) => Request::AgentWait {
            target,
            timeout_secs: timeout,
        },
        Command::Agent(AgentCommand::List) => Request::AgentList,
        Command::Tasks(TasksCommand::Start { kind, file, shared }) => Request::TasksStart {
            from: caller_pane(),
            cwd: std::env::current_dir().ok(),
            kind,
            shared,
            tasks: read_tasks(file)?,
        },
        Command::Tasks(TasksCommand::Wait { batch, timeout }) => Request::TasksWait {
            batch,
            from: caller_pane(),
            timeout_secs: timeout,
        },
        Command::Tasks(TasksCommand::List { batch }) => Request::TasksWait {
            batch,
            from: caller_pane(),
            timeout_secs: 0,
        },
        Command::Tasks(TasksCommand::Clean { batch, force }) => Request::TasksClean {
            batch,
            force,
            from: caller_pane(),
        },
        Command::Task(TaskCommand::Done {
            summary,
            summary_file,
            failed,
            no_changes,
        }) => Request::TaskDone {
            task: task_env()?,
            pane: caller_pane(),
            summary: match (summary, summary_file) {
                (Some(text), _) => text,
                (None, Some(path)) => {
                    let bytes =
                        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    String::from_utf8(bytes)
                        .map_err(|_| "the summary file must be UTF-8 text")?
                        .trim_start_matches('\u{feff}')
                        .to_string()
                }
                (None, None) => return Err("give --summary or --summary-file".into()),
            },
            failed,
            no_changes,
        },
        Command::Skill { .. } | Command::Hook { .. } | Command::Statusline { .. } => {
            return Err("not a request".into());
        }
    })
}

fn call(request: &Request) -> Result<serde_json::Value, String> {
    // Each Solace launch picks a new pipe name and hands it to its shells.
    let name = std::env::var("SOLACE_PIPE")
        .ok()
        .filter(|name| !name.is_empty())
        .ok_or("not inside a Solace terminal (SOLACE_PIPE is not set)")?;
    // A pipe name on Windows, a socket path on Unix (see solace_infra::ipc::endpoint).
    #[cfg(windows)]
    let name = {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        name.as_str().to_ns_name::<GenericNamespaced>()
    };
    #[cfg(unix)]
    let name = {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        name.as_str().to_fs_name::<GenericFilePath>()
    };
    let name = name.map_err(|e| e.to_string())?;
    let stream = Stream::connect(name)
        .map_err(|err| format!("could not reach Solace ({err}); is the app still running?"))?;
    let mut line = serde_json::to_vec(request).map_err(|e| e.to_string())?;
    line.push(b'\n');
    (&stream).write_all(&line).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    serde_json::from_str::<Response>(&reply).map_err(|e| format!("bad reply from Solace: {e}"))?
}

fn install_skill() -> Result<serde_json::Value, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("no home directory")?;
    // Codex and several other agents read ~/.agents/skills.
    let mut installed = Vec::new();
    for folder in [".claude/skills/solace", ".agents/skills/solace"] {
        let dir = PathBuf::from(&home).join(folder);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("SKILL.md");
        std::fs::write(&path, SKILL).map_err(|e| e.to_string())?;
        installed.push(path);
    }
    Ok(serde_json::json!({ "installed": installed }))
}

/// Hooks must never get in Claude Code's way: outside Solace, or on any error, this does nothing
/// and exits 0 with no output (a hook's output can reach the model).
fn hook() -> ExitCode {
    let mut input = String::new();
    let _ = std::io::stdin()
        .take(1024 * 1024)
        .read_to_string(&mut input);
    // Claude Code writes the whole event; leaving the rest unread would break its pipe.
    let _ = std::io::copy(&mut std::io::stdin(), &mut std::io::sink());
    let (Some(pane), Ok(event)) = (
        caller_pane(),
        serde_json::from_str::<serde_json::Value>(&input),
    ) else {
        return ExitCode::SUCCESS;
    };
    let state = match claude_hook(&event) {
        HookVerdict::Report(state) => Some(state),
        HookVerdict::Clear => None,
        HookVerdict::Ignore => return ExitCode::SUCCESS,
    };
    let session = event
        .get("session_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let _ = call(&Request::AgentReport {
        pane,
        session,
        state,
    });
    ExitCode::SUCCESS
}

/// Claude Code's status line: always exits 0 and prints nothing but the `--tee` passthrough.
fn statusline(tee: bool) -> ExitCode {
    let mut input = Vec::new();
    let _ = std::io::stdin().take(1024 * 1024).read_to_end(&mut input);
    if tee {
        let mut out = std::io::stdout();
        let _ = out.write_all(&input);
        let _ = out.flush();
    }
    if let Ok(status) = serde_json::from_slice::<serde_json::Value>(&input) {
        let (sender, done) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = call(&Request::UsageReport {
                pane: caller_pane(),
                status,
            });
            let _ = sender.send(());
        });
        // Don't hold up the status line if Solace is slow.
        let _ = done.recv_timeout(std::time::Duration::from_millis(300));
    }
    ExitCode::SUCCESS
}

/// Run by the uninstaller: takes Solace's hooks and skill out of the agents' configs. Never fails
/// the uninstall, and prints nothing.
fn remove_integration(home: &std::path::Path) {
    let _ = solace_infra::integrations::uninstall_claude(
        &solace_infra::integrations::ClaudePaths::in_home(home),
    );
}

fn main() -> ExitCode {
    // Before clap: a usage error exits 2, which Claude Code reads as "block this".
    match std::env::args().nth(1).as_deref() {
        Some("integration") => {
            if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))
            {
                remove_integration(std::path::Path::new(&home));
            }
            return ExitCode::SUCCESS;
        }
        Some("hook") => return hook(),
        Some("statusline") => return statusline(std::env::args().any(|a| a == "--tee")),
        _ => {}
    }
    let command = Cli::parse().command;
    // `tasks list` only looks: unfinished tasks are no reason to fail.
    let is_list = matches!(command, Command::Tasks(TasksCommand::List { .. }));
    let result = match command {
        Command::Skill { action: None } => {
            print!("{SKILL}");
            return ExitCode::SUCCESS;
        }
        Command::Skill { action: Some(a) } if a == "install" => install_skill(),
        Command::Skill { action: Some(a) } => {
            Err(format!("unknown skill action {a:?}; try install"))
        }
        command => request(command).and_then(|r| call(&r)),
    };
    match result {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            let timed_out =
                value.get("timedOut").and_then(serde_json::Value::as_bool) == Some(true);
            let unclean = value
                .get("failed")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|f| !f.is_empty());
            // A wait that gave up: the agent must run it again.
            if timed_out && !is_list {
                ExitCode::from(2)
            } else if unclean {
                // `tasks clean` could not remove everything.
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(message) => {
            eprintln!("solace: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn integration_remove_strips_what_install_added() {
        use solace_infra::integrations::{ClaudePaths, claude_installed, install_claude};
        let home = std::env::temp_dir().join(format!("solace-cli-remove-{}", std::process::id()));
        let paths = ClaudePaths::in_home(&home);
        std::fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        let exe = home.join("solace.exe");
        std::fs::write(&exe, "").unwrap();
        install_claude(&paths, &exe).unwrap();
        assert!(claude_installed(&paths));
        super::remove_integration(&home);
        assert!(!claude_installed(&paths) && !paths.agents_skill.exists());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
