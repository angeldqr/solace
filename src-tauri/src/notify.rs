use std::time::Duration;

use solace_core::agents::AgentState;

/// Shorter spells of "working" are redraws or quick replies, not finished tasks worth a ping.
const MIN_WORK: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq)]
pub enum Notice {
    NeedsYou,
    Finished,
}

/// What a state change is worth telling the user about, if anything.
pub fn notice(prev: AgentState, next: AgentState, worked: Duration) -> Option<Notice> {
    match (prev, next) {
        (AgentState::Blocked, _) => None,
        (_, AgentState::Blocked) => Some(Notice::NeedsYou),
        (AgentState::Working, AgentState::Idle) if worked >= MIN_WORK => Some(Notice::Finished),
        _ => None,
    }
}

/// Title and body in the UI's language ("es" or anything else for English).
pub fn text(
    notice: &Notice,
    agent: &str,
    label: Option<&str>,
    space: &str,
    locale: &str,
) -> (String, String) {
    let who = match label {
        Some(label) => format!("{label} ({agent})"),
        None => agent.to_string(),
    };
    let spanish = locale == "es";
    match (notice, spanish) {
        (Notice::NeedsYou, false) => (
            format!("{who} needs you"),
            format!("Waiting for your answer in {space}"),
        ),
        (Notice::NeedsYou, true) => (
            format!("{who} te necesita"),
            format!("Esperando tu respuesta en {space}"),
        ),
        (Notice::Finished, false) => (format!("{who} finished"), format!("Done in {space}")),
        (Notice::Finished, true) => (format!("{who} terminó"), format!("Listo en {space}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentState::*;

    #[test]
    fn notifies_on_needs_you_and_on_real_work_finishing() {
        let long = Duration::from_secs(40);
        assert_eq!(notice(Working, Blocked, long), Some(Notice::NeedsYou));
        assert_eq!(notice(Idle, Blocked, long), Some(Notice::NeedsYou));
        assert_eq!(notice(Blocked, Blocked, long), None);
        assert_eq!(notice(Working, Idle, long), Some(Notice::Finished));
        assert_eq!(
            notice(Working, Idle, Duration::from_secs(1)),
            None,
            "a redraw is not a task"
        );
        assert_eq!(
            notice(Blocked, Idle, long),
            None,
            "the user just answered it"
        );
        assert_eq!(notice(Idle, Working, long), None);
    }

    #[test]
    fn speaks_the_ui_language() {
        let (title, body) = text(&Notice::NeedsYou, "Codex", Some("reviewer"), "solace", "es");
        assert_eq!(title, "reviewer (Codex) te necesita");
        assert_eq!(body, "Esperando tu respuesta en solace");
        assert_eq!(
            text(&Notice::Finished, "Claude Code", None, "api", "en").0,
            "Claude Code finished"
        );
    }
}
