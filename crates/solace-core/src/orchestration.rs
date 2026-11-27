//! The local API that lets agents drive Solace through the `solace` CLI: open panes, start other
//! agents, hand them prompts and read what they answered.

use serde::{Deserialize, Serialize};

use crate::tasks::TaskSpec;
use crate::terminals::{Direction, PaneId};

/// Pipe (Windows) or socket name. Unique per launch: a fixed name could be claimed first by
/// another local account, which would then receive every prompt. Shells inside Solace get it
/// through `SOLACE_PIPE`.
pub fn endpoint_name(user: &str, nonce: u64) -> String {
    format!("solace-{user}-{nonce:016x}.sock")
}

/// One request per connection, sent as a single JSON line; the reply is one JSON line too.
/// `target` accepts a pane id or the name given to an agent at start.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Request {
    #[serde(rename = "pane.list")]
    PaneList,
    #[serde(rename = "pane.split")]
    PaneSplit {
        pane: Option<PaneId>,
        direction: Direction,
    },
    #[serde(rename = "pane.send")]
    PaneSend {
        target: String,
        text: String,
        enter: bool,
    },
    #[serde(rename = "pane.read")]
    PaneRead {
        target: String,
        lines: Option<usize>,
    },
    #[serde(rename = "pane.close")]
    PaneClose { target: String },
    #[serde(rename = "agent.start")]
    AgentStart {
        name: String,
        kind: String,
        from: Option<PaneId>,
        direction: Direction,
        prompt: Option<String>,
    },
    #[serde(rename = "agent.prompt")]
    AgentPrompt {
        target: String,
        text: String,
        wait: bool,
        timeout_secs: u64,
    },
    #[serde(rename = "agent.wait")]
    AgentWait { target: String, timeout_secs: u64 },
    #[serde(rename = "agent.list")]
    AgentList,
    /// From an agent's own hooks: its exact state (`None` to go back to reading the screen).
    #[serde(rename = "agent.report")]
    AgentReport {
        pane: PaneId,
        /// The reporting Claude session: a nested `claude` in the same pane must not overwrite it.
        session: String,
        state: Option<crate::agents::AgentState>,
    },
    /// Starts one worker agent per task, each in its own git worktree and pane.
    #[serde(rename = "tasks.start")]
    TasksStart {
        from: Option<PaneId>,
        /// Folder the CLI ran in; the lead pane's folder when absent.
        cwd: Option<std::path::PathBuf>,
        /// Agent kind for tasks that name none, or "auto" for the one with the most room left.
        kind: Option<String>,
        /// Allow a folder that is not a git repository: the workers then edit it together.
        #[serde(default)]
        shared: bool,
        tasks: Vec<TaskSpec>,
    },
    /// Waits until every task of a batch (default: this pane's newest) is finished.
    #[serde(rename = "tasks.wait")]
    TasksWait {
        batch: Option<String>,
        from: Option<PaneId>,
        timeout_secs: u64,
    },
    /// Closes a batch's panes and removes its worktrees; the branches stay.
    #[serde(rename = "tasks.clean")]
    TasksClean {
        batch: String,
        force: bool,
        /// The pane that asked: never closed by the clean-up.
        #[serde(default)]
        from: Option<PaneId>,
    },
    /// From a worker: its task is finished.
    #[serde(rename = "task.done")]
    TaskDone {
        task: String,
        pane: Option<PaneId>,
        summary: String,
        failed: bool,
    },
    /// From Claude Code's status line: the raw status JSON, which carries the rate limits.
    #[serde(rename = "usage.report")]
    UsageReport {
        pane: Option<PaneId>,
        status: serde_json::Value,
    },
}

pub type Response = Result<serde_json::Value, String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_use_dotted_method_names() {
        let json = serde_json::to_string(&Request::PaneList).unwrap();
        assert_eq!(json, r#"{"method":"pane.list"}"#);
        let parsed: Request = serde_json::from_str(
            r#"{"method":"agent.prompt","params":{"target":"reviewer","text":"hi","wait":true,"timeout_secs":5}}"#,
        )
        .unwrap();
        assert!(matches!(parsed, Request::AgentPrompt { wait: true, .. }));
        let report = Request::UsageReport {
            pane: Some(PaneId(3)),
            status: serde_json::json!({"a": 1}),
        };
        let back: Request = serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert!(matches!(
            back,
            Request::UsageReport {
                pane: Some(PaneId(3)),
                ..
            }
        ));
    }

    #[test]
    fn task_requests_round_trip() {
        let start = Request::TasksStart {
            from: Some(PaneId(1)),
            cwd: Some("C:/repo".into()),
            kind: Some("auto".into()),
            shared: false,
            tasks: vec![TaskSpec {
                title: "t".into(),
                prompt: "p".into(),
                kind: None,
                base: None,
            }],
        };
        let json = serde_json::to_string(&start).unwrap();
        assert!(json.contains(r#""method":"tasks.start""#), "{json}");
        let Request::TasksStart { tasks, cwd, .. } = serde_json::from_str(&json).unwrap() else {
            panic!("not tasks.start");
        };
        assert_eq!((tasks.len(), cwd), (1, Some("C:/repo".into())));
        for (json, ok) in [
            (
                r#"{"method":"tasks.wait","params":{"batch":null,"from":2,"timeout_secs":0}}"#,
                true,
            ),
            (
                r#"{"method":"tasks.clean","params":{"batch":"b1","force":false}}"#,
                true,
            ),
            (
                r#"{"method":"task.done","params":{"task":"b1-1","pane":null,"summary":"s","failed":true}}"#,
                true,
            ),
            (r#"{"method":"task.done","params":{"task":"b1-1"}}"#, false),
        ] {
            assert_eq!(serde_json::from_str::<Request>(json).is_ok(), ok, "{json}");
        }
    }
}
