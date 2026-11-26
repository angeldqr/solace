//! The local API that lets agents drive Solace through the `solace` CLI: open panes, start other
//! agents, hand them prompts and read what they answered.

use serde::{Deserialize, Serialize};

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
}
