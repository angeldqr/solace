pub mod hooks;

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
    /// Where the agent prints how full its context is, for agents with no better source.
    context: Option<ContextRule>,
}

/// A footer pattern with named groups: `used` ("11.0K", "1.2M", "8500"), `pct` (percent used)
/// and/or `left` (percent left).
#[derive(Debug, Deserialize)]
struct ContextRule {
    region: Region,
    #[serde(deserialize_with = "regex")]
    regex: Regex,
}

/// Context use as an agent's own screen shows it; either part may be missing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenContext {
    pub used: Option<u64>,
    pub percent: Option<f64>,
    /// The window size, when the screen prints it.
    pub limit: Option<u64>,
}

/// "11.0K" -> 11000, "1.2M" -> 1200000, "8500" -> 8500.
fn parse_count(s: &str) -> Option<u64> {
    let (num, scale) = match s.chars().last()? {
        'K' | 'k' => (&s[..s.len() - 1], 1e3),
        'M' | 'm' => (&s[..s.len() - 1], 1e6),
        _ => (s, 1.0),
    };
    Some((num.parse::<f64>().ok()? * scale).round() as u64)
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

/// Like `identify`, also seeing through interpreters: npm installs agents as scripts that run as
/// `node /usr/local/bin/opencode`, with no node_modules path in the command line.
pub fn identify_process(name: &str, args: &[String]) -> Option<&'static AgentDef> {
    let base = |arg: &str| {
        let file = arg.rsplit(['/', '\\']).next().unwrap_or(arg).to_lowercase();
        file.strip_suffix(".exe")
            .map_or(file.clone(), str::to_string)
    };
    let program = args.first().map(|a| base(a));
    let interpreted = program.as_deref().is_some_and(|p| {
        ["node", "bun", "deno", "sh", "bash"].contains(&p) || p.starts_with("python")
    });
    let script = interpreted
        .then(|| args.iter().skip(1).find(|a| !a.starts_with('-')))
        .flatten()
        .map(|a| base(a));
    std::iter::once(name.to_string())
        .chain(program)
        .chain(script)
        .find_map(|exe| identify(&exe, args))
}

impl AgentDef {
    /// Whether its screen can be read; without rules only output activity is known.
    pub fn has_rules(&self) -> bool {
        !self.rules.is_empty()
    }

    /// The command that starts this agent in a shell.
    pub fn command(&self) -> Option<&str> {
        self.executables.first().map(String::as_str)
    }

    /// How full the context is according to the screen footer; `None` without a rule or a match.
    pub fn context_from_screen(&self, rows: &[String]) -> Option<ScreenContext> {
        let rule = self.context.as_ref()?;
        let text = rule.region.text("", rows);
        // The newest report is the lowest on screen.
        let caps = rule.regex.captures_iter(&text).last()?;
        let num = |name: &str| caps.name(name).and_then(|m| m.as_str().parse::<f64>().ok());
        let count = |name: &str| caps.name(name).and_then(|m| parse_count(m.as_str()));
        let used = count("used");
        let percent = num("pct").or_else(|| num("left").map(|left| 100.0 - left));
        (used.is_some() || percent.is_some()).then_some(ScreenContext {
            used,
            percent,
            limit: count("limit"),
        })
    }

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
    fn identifies_agents_run_by_an_interpreter() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let opencode = identify_process("node", &args("node /usr/local/bin/opencode --yolo"));
        assert_eq!(opencode.unwrap().id, "opencode");
        assert_eq!(identify_process("claude", &[]).unwrap().id, "claude");
        // Only an interpreter's script counts, not any argument of any program.
        assert!(identify_process("vim", &args("vim opencode")).is_none());
        assert!(identify_process("node", &args("node server.js")).is_none());
    }

    #[test]
    fn identifies_native_and_node_agents_but_not_installs() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert_eq!(identify("C:\\bin\\claude.exe", &[]).unwrap().id, "claude");
        assert_eq!(identify("Codex.EXE", &[]).unwrap().id, "codex");
        let node = args(
            r"node.exe C:\Users\me\AppData\Roaming\npm\node_modules\opencode-ai\bin\opencode.js",
        );
        assert_eq!(identify("node.exe", &node).unwrap().id, "opencode");
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

        let trust = screen(&[
            "Do you trust the files in this folder?",
            "❯ 1. Yes, proceed",
            "  2. No, exit",
            "Enter to confirm · Esc to exit",
        ]);
        assert_eq!(claude.detect("", &trust), Some(AgentState::Blocked));

        let busy = screen(&["✻ Thinking… (3s · esc to interrupt)", "", "❯ "]);
        assert_eq!(claude.detect("", &busy), Some(AgentState::Working));
    }

    #[test]
    fn context_is_read_from_the_footer() {
        let opencode = agent("opencode");
        let ctx = opencode
            .context_from_screen(&screen(&["answer", "  ctrl+p commands   11.0K (1%)"]))
            .unwrap();
        assert_eq!((ctx.used, ctx.percent), (Some(11_000), Some(1.0)));
        let big = opencode
            .context_from_screen(&screen(&["1.2M (45.5%)"]))
            .unwrap();
        assert_eq!((big.used, big.percent), (Some(1_200_000), Some(45.5)));
        assert!(
            opencode
                .context_from_screen(&screen(&["nothing here"]))
                .is_none()
        );

        let ctx = agent("codex")
            .context_from_screen(&screen(&["gpt-5 · 83% context left · ~/proj"]))
            .unwrap();
        assert_eq!((ctx.used, ctx.percent), (None, Some(17.0)));
        assert!(
            agent("claude")
                .context_from_screen(&screen(&["11.0K (1%)"]))
                .is_none()
        );
        // Claude's /context output; the later report wins.
        let ctx = agent("claude")
            .context_from_screen(&screen(&[
                "701.3k/1m tokens (70%)",
                "> /compact",
                "    claude-opus-5-5",
                "    60.3k/1m tokens (6%)",
                "  Free space: 906.7k (91%)",
            ]))
            .unwrap();
        assert_eq!(
            (ctx.used, ctx.percent, ctx.limit),
            (Some(60_300), Some(6.0), Some(1_000_000))
        );
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
