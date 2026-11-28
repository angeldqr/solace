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

/// Codex hook events (`~/.codex/hooks.json`); it has no Notification event, but tells when it asks.
pub const CODEX_EVENTS: &[&str] = &[
    "UserPromptSubmit",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
    "Interrupt",
    "SessionEnd",
];

/// Kimi Code hook events (`[[hooks]]` in `~/.kimi-code/config.toml`).
pub const KIMI_EVENTS: &[&str] = &[
    "UserPromptSubmit",
    "PostToolUse",
    "PermissionRequest",
    "PermissionResult",
    "Stop",
    "StopFailure",
    "Interrupt",
    "SessionEnd",
];

/// What a hook payload from `agent` ("claude", "codex", "kimi") means for the pane's state.
pub fn agent_hook(agent: &str, event: &Value) -> HookVerdict {
    if agent == "claude" {
        return claude_hook(event);
    }
    let name = event
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match name {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PermissionResult" => {
            HookVerdict::Report(AgentState::Working)
        }
        "PermissionRequest" => HookVerdict::Report(AgentState::Blocked),
        "Stop" | "StopFailure" | "Interrupt" => HookVerdict::Report(AgentState::Idle),
        "SessionEnd" => HookVerdict::Clear,
        _ => HookVerdict::Ignore,
    }
}

/// Antigravity reruns its status line on every state change; its JSON says what the agent is doing.
pub fn antigravity_state(status: &Value) -> Option<AgentState> {
    if status
        .get("tool_confirmation_pending")
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Some(AgentState::Blocked);
    }
    match status.get("agent_state")?.as_str()? {
        "idle" => Some(AgentState::Idle),
        "working" | "tool_use" | "reviewing" => Some(AgentState::Working),
        _ => None,
    }
}

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
        for (agent, events) in [("codex", CODEX_EVENTS), ("kimi", KIMI_EVENTS)] {
            for name in events {
                let event = json!({"hook_event_name": name});
                assert_ne!(
                    agent_hook(agent, &event),
                    HookVerdict::Ignore,
                    "{agent} {name}"
                );
            }
        }
    }

    #[test]
    fn reads_antigravity_status() {
        let s = |v: Value| antigravity_state(&v);
        assert_eq!(
            s(json!({"agent_state": "tool_use", "tool_confirmation_pending": true})),
            Some(AgentState::Blocked)
        );
        assert_eq!(
            s(json!({"agent_state": "reviewing"})),
            Some(AgentState::Working)
        );
        assert_eq!(s(json!({"agent_state": "idle"})), Some(AgentState::Idle));
        assert_eq!(s(json!({"agent_state": "authenticating"})), None);
    }

    #[test]
    fn maps_codex_and_kimi_events() {
        let v = |agent, name| agent_hook(agent, &json!({ "hook_event_name": name }));
        assert_eq!(
            v("codex", "PermissionRequest"),
            HookVerdict::Report(AgentState::Blocked)
        );
        assert_eq!(
            v("kimi", "PermissionResult"),
            HookVerdict::Report(AgentState::Working)
        );
        assert_eq!(
            v("kimi", "Interrupt"),
            HookVerdict::Report(AgentState::Idle)
        );
        assert_eq!(v("codex", "SessionEnd"), HookVerdict::Clear);
        assert_eq!(v("codex", "SessionHeartbeat"), HookVerdict::Ignore);
        // Claude's own rules still apply under its name.
        assert_eq!(v("claude", "PermissionRequest"), HookVerdict::Ignore);
    }
}
