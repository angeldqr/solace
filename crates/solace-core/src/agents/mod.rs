use std::borrow::Cow;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    Idle,
    Working,
    /// Waiting for the user: a permission prompt, a question, a trust dialog.
    Blocked,
}

/// A CLI agent Solace knows how to recognize, from `catalog.toml`.
#[derive(Debug, Deserialize)]
pub struct AgentDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    executables: Vec<String>,
    #[serde(default)]
    packages: Vec<String>,
    #[serde(default, rename = "rule")]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
struct Rule {
    state: AgentState,
    region: Region,
    #[serde(deserialize_with = "regex")]
    regex: Regex,
    #[serde(default, deserialize_with = "optional_regex")]
    not: Option<Regex>,
}

#[derive(Debug)]
enum Region {
    Title,
    /// The last N non-empty screen lines.
    Bottom(usize),
}

impl<'de> Deserialize<'de> for Region {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw == "title" {
            return Ok(Region::Title);
        }
        raw.strip_prefix("bottom:")
            .and_then(|n| n.parse().ok())
            .map(Region::Bottom)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown region {raw:?}")))
    }
}

fn regex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Regex, D::Error> {
    Regex::new(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

fn optional_regex<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Regex>, D::Error> {
    regex(deserializer).map(Some)
}

#[derive(Deserialize)]
struct Catalog {
    agent: Vec<AgentDef>,
}

pub fn catalog() -> &'static [AgentDef] {
    static CATALOG: OnceLock<Vec<AgentDef>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        toml::from_str::<Catalog>(include_str!("catalog.toml"))
            .expect("built-in agent catalog is valid (checked by tests)")
            .agent
    })
}

/// The agent a process is, given its executable file name and command line.
pub fn identify(exe: &str, args: &[String]) -> Option<&'static AgentDef> {
    let file = exe.rsplit(['/', '\\']).next().unwrap_or(exe).to_lowercase();
    let stem = file.strip_suffix(".exe").unwrap_or(&file);
    // Script CLIs run as `node …/node_modules/<package>/cli.js`; requiring the node_modules path
    // keeps `npm install <package>` from looking like the agent itself.
    let command = args.join(" ").replace('\\', "/").to_lowercase();
    catalog().iter().find(|agent| {
        agent.executables.iter().any(|e| e == stem)
            || agent
                .packages
                .iter()
                .any(|p| command.contains(&format!("node_modules/{p}")))
    })
}

impl AgentDef {
    /// The state the screen shows, from the first matching rule; `None` when no rule applies.
    pub fn detect(&self, title: &str, rows: &[String]) -> Option<AgentState> {
        self.rules
            .iter()
            .find(|rule| {
                let text = rule.region.text(title, rows);
                rule.regex.is_match(&text) && !rule.not.as_ref().is_some_and(|n| n.is_match(&text))
            })
            .map(|rule| rule.state)
    }
}

impl Region {
    fn text<'a>(&self, title: &'a str, rows: &[String]) -> Cow<'a, str> {
        match self {
            Region::Title => Cow::Borrowed(title),
            Region::Bottom(n) => {
                let mut lines: Vec<&str> = rows
                    .iter()
                    .map(|r| r.trim_end())
                    .filter(|r| !r.is_empty())
                    .rev()
                    .take(*n)
                    .collect();
                lines.reverse();
                Cow::Owned(lines.join("\n"))
            }
        }
    }
}

/// Spinners flicker between frames, so leaving Working or Blocked needs the new verdict to hold
/// this long; entering them is immediate so the UI reacts at once.
const SETTLE: Duration = Duration::from_millis(800);

#[derive(Debug)]
pub struct StateTracker {
    pub state: AgentState,
    pending: Option<(AgentState, Instant)>,
}

impl Default for StateTracker {
    fn default() -> Self {
        Self {
            state: AgentState::Idle,
            pending: None,
        }
    }
}

impl StateTracker {
    /// Feeds one detection; true when the reported state changed.
    pub fn observe(&mut self, verdict: AgentState, now: Instant) -> bool {
        if verdict == self.state {
            self.pending = None;
            return false;
        }
        let settled = match self.pending {
            Some((pending, since)) if pending == verdict => now.duration_since(since) >= SETTLE,
            _ => {
                self.pending = Some((verdict, now));
                false
            }
        };
        if verdict != AgentState::Idle || settled {
            self.state = verdict;
            self.pending = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    fn agent(id: &str) -> &'static AgentDef {
        catalog().iter().find(|a| a.id == id).unwrap()
    }

    #[test]
    fn catalog_parses_with_unique_ids() {
        let ids: Vec<_> = catalog().iter().map(|a| a.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(ids.len(), unique.len());
        assert!(ids.contains(&"claude") && ids.contains(&"codex"));
    }

    #[test]
    fn identifies_native_and_node_agents_but_not_installs() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(identify("C:\\bin\\claude.exe", &[]).unwrap().id, "claude");
        assert_eq!(identify("Codex.EXE", &[]).unwrap().id, "codex");
        let node = args(
            r"node.exe C:\Users\me\AppData\Roaming\npm\node_modules\@google\gemini-cli\dist\index.js",
        );
        assert_eq!(identify("node.exe", &node).unwrap().id, "gemini");
        assert!(
            identify(
                "node.exe",
                &args("npm install -g @anthropic-ai/claude-code")
            )
            .is_none()
        );
        assert!(identify("pwsh.exe", &[]).is_none());
    }

    #[test]
    fn claude_states_follow_title_and_prompts() {
        let claude = agent("claude");
        let idle_box = screen(&["Done.", "╭────╮", "│ ❯  │", "╰────╯"]);
        assert_eq!(
            claude.detect("⠂ Refactoring", &idle_box),
            Some(AgentState::Working)
        );
        assert_eq!(
            claude.detect("✳ Claude Code", &idle_box),
            Some(AgentState::Idle)
        );

        let prompt = screen(&[
            "Bash command",
            "  rm -rf target",
            "Do you want to proceed?",
            "❯ 1. Yes",
            "  2. No",
            "Esc to cancel",
        ]);
        assert_eq!(
            claude.detect("✳ Claude Code", &prompt),
            Some(AgentState::Blocked)
        );

        let busy = screen(&["✻ Thinking… (3s · esc to interrupt)", "", "❯ "]);
        assert_eq!(claude.detect("", &busy), Some(AgentState::Working));
    }

    #[test]
    fn codex_reads_its_title() {
        let codex = agent("codex");
        assert_eq!(
            codex.detect("Action Required", &[]),
            Some(AgentState::Blocked)
        );
        assert_eq!(codex.detect("⠙ codex", &[]), Some(AgentState::Working));
        assert_eq!(codex.detect("codex", &[]), Some(AgentState::Idle));
    }

    #[test]
    fn tracker_enters_busy_states_at_once_and_settles_into_idle() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut tracker = StateTracker::default();

        assert!(tracker.observe(AgentState::Working, at(0)));
        assert!(
            !tracker.observe(AgentState::Idle, at(100)),
            "a spinner gap is not idle"
        );
        assert!(!tracker.observe(AgentState::Working, at(200)));
        assert!(!tracker.observe(AgentState::Idle, at(300)));
        assert!(!tracker.observe(AgentState::Idle, at(900)));
        assert!(tracker.observe(AgentState::Idle, at(1200)));
        assert_eq!(tracker.state, AgentState::Idle);
        assert!(tracker.observe(AgentState::Blocked, at(1300)));
    }
}
