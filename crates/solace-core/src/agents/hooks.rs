//! Claude Code hook events, turned into the agent state they announce. Hooks report exact
//! transitions (a permission dialog opening, a turn ending) that screen reading can only infer.

use serde_json::Value;

use super::AgentState;

#[derive(Debug, PartialEq)]
pub enum HookVerdict {
    Report(AgentState),
    /// The session ended: forget what was reported and go back to reading the screen.
    Clear,
    Ignore,
}

/// The Claude Code hook events Solace listens to; `solace hook` handles every one of them.
/// Not PreToolUse or PermissionRequest: on Windows a failing hook there blocks the tool call.
pub const CLAUDE_EVENTS: &[&str] = &[
    "UserPromptSubmit",
    "PostToolUse",
    "Notification",
    "Stop",
    "SessionEnd",
];

/// What a Claude Code hook payload means for the pane's state.
pub fn claude_hook(event: &Value) -> HookVerdict {
    let name = event
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match name {
        "UserPromptSubmit" | "PostToolUse" => HookVerdict::Report(AgentState::Working),
        "Notification" => match event.get("notification_type").and_then(Value::as_str) {
            Some("permission_prompt" | "elicitation_dialog") => {
                HookVerdict::Report(AgentState::Blocked)
            }
            Some("idle_prompt") => HookVerdict::Report(AgentState::Idle),
            _ => HookVerdict::Ignore,
        },
        // A subagent stopping is not the session finishing.
        "Stop" if event.get("agent_id").is_none_or(Value::is_null) => {
            HookVerdict::Report(AgentState::Idle)
        }
        "SessionEnd" => HookVerdict::Clear,
        _ => HookVerdict::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_claude_events_to_states() {
        let v = |e: Value| claude_hook(&e);
        assert_eq!(
            v(json!({"hook_event_name": "UserPromptSubmit"})),
            HookVerdict::Report(AgentState::Working)
        );
        assert_eq!(
            v(json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"})),
            HookVerdict::Report(AgentState::Blocked)
        );
        assert_eq!(
            v(json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"})),
            HookVerdict::Report(AgentState::Idle)
        );
        assert_eq!(
            v(json!({"hook_event_name": "Notification", "notification_type": "auth_success"})),
            HookVerdict::Ignore
        );
        assert_eq!(
            v(json!({"hook_event_name": "Stop"})),
            HookVerdict::Report(AgentState::Idle)
        );
        assert_eq!(
            v(json!({"hook_event_name": "SessionEnd"})),
            HookVerdict::Clear
        );
        assert_eq!(
            v(json!({"hook_event_name": "Stop", "agent_id": "a1"})),
            HookVerdict::Ignore,
            "a subagent finishing is not the session"
        );
        assert_eq!(v(json!({"nothing": true})), HookVerdict::Ignore);
    }

    #[test]
    fn every_listened_event_means_something() {
        for name in CLAUDE_EVENTS {
            let event = json!({"hook_event_name": name, "notification_type": "permission_prompt"});
            assert_ne!(claude_hook(&event), HookVerdict::Ignore, "{name}");
        }
    }
}
