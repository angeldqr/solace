//! Usage meter: Claude's rate limits arrive from its status line, Codex's and the token totals
//! are read from the files those tools keep.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use solace_core::terminals::PaneId;
use solace_core::usage::{ClaudeSession, Limit, ProviderUsage, Tokens, parse_claude_status};
use solace_infra::usage::{ClaudeLog, CodexCache};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, Shared};

const EVERY: Duration = Duration::from_secs(30);
const FIVE_HOURS: u64 = 5 * 3600;

#[derive(Default)]
pub struct UsageState {
    claude: Option<ProviderUsage>,
    codex: Option<ProviderUsage>,
    sessions: HashMap<PaneId, ClaudeSession>,
    tokens: TokensView,
}

#[derive(Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TokensView {
    block_start: u64,
    block: Tokens,
    hourly: Vec<Hour>,
}

#[derive(Clone, PartialEq, Serialize)]
struct Hour {
    hour: u64,
    tokens: Tokens,
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionUsage {
    pane: PaneId,
    #[serde(flatten)]
    session: ClaudeSession,
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageView {
    claude: Option<ProviderUsage>,
    codex: Option<ProviderUsage>,
    sessions: Vec<SessionUsage>,
    tokens: TokensView,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn home_dir(var: &str, default: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from).or_else(|| {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(home).join(default))
    })
}

/// Panes of closed terminals drop out here, so a finished session never lingers.
pub fn view(state: &AppState) -> UsageView {
    let mut sessions: Vec<SessionUsage> = state
        .usage
        .sessions
        .iter()
        .filter(|(pane, _)| state.ptys.contains_key(pane))
        .map(|(pane, session)| SessionUsage {
            pane: *pane,
            session: session.clone(),
        })
        .collect();
    sessions.sort_by_key(|s| s.pane.0);
    UsageView {
        claude: state.usage.claude.clone(),
        codex: state.usage.codex.clone(),
        sessions,
        tokens: state.usage.tokens.clone(),
    }
}

/// How full a provider's fullest limit is, ignoring windows that have already reset. `None`
/// when nothing is known (and always for agents whose usage Solace doesn't read).
pub fn used_percent(state: &AppState, kind: &str) -> Option<f64> {
    let usage = match kind {
        "claude" => &state.usage.claude,
        "codex" => &state.usage.codex,
        _ => return None,
    };
    let now = now_secs();
    usage
        .as_ref()?
        .limits
        .iter()
        .filter(|l| l.resets_at.is_none_or(|at| at > now))
        .map(|l| l.used_percent)
        .reduce(f64::max)
}

#[tauri::command]
pub fn usage_get(state: State<Shared>) -> UsageView {
    view(&state.lock().unwrap())
}

/// Keeps the new report only if it says something new, so the status line firing every few
/// hundred milliseconds doesn't wake the UI.
fn merge(
    slot: &mut Option<ProviderUsage>,
    limits: Option<Vec<Limit>>,
    plan: Option<String>,
    at: u64,
) -> bool {
    let Some(limits) = limits else { return false };
    if slot
        .as_ref()
        .is_some_and(|u| u.limits == limits && (plan.is_none() || u.plan == plan))
    {
        return false;
    }
    let plan = plan.or_else(|| slot.as_ref().and_then(|u| u.plan.clone()));
    *slot = Some(ProviderUsage {
        limits,
        plan,
        updated_at: at,
    });
    true
}

/// A `solace statusline` report from a Claude Code pane.
pub fn report(app: &AppHandle, pane: Option<PaneId>, status: &Value) {
    let (limits, session) = parse_claude_status(status);
    let state = app.state::<Shared>();
    let mut state = state.lock().unwrap();
    let mut changed = merge(&mut state.usage.claude, limits, None, now_secs() * 1000);
    if let Some(pane) = pane
        && state.usage.sessions.get(&pane) != Some(&session)
    {
        state.usage.sessions.insert(pane, session);
        changed = true;
    }
    if changed {
        let _ = app.emit("usage", view(&state));
    }
}

pub fn watch(app: AppHandle) {
    let mut log = ClaudeLog::default();
    let mut codex = CodexCache::default();
    let codex_home = home_dir("CODEX_HOME", ".codex");
    let projects = home_dir("CLAUDE_CONFIG_DIR", ".claude").map(|d| d.join("projects"));
    loop {
        // File reads happen outside the state lock.
        let now = now_secs();
        let codex_usage = codex_home.as_deref().and_then(|h| codex.read(h));
        if let Some(p) = &projects {
            log.scan(p, now);
        }
        let reset = app
            .state::<Shared>()
            .lock()
            .unwrap()
            .usage
            .claude
            .as_ref()
            .and_then(|u| u.limits.iter().find(|l| l.window_minutes == 300)?.resets_at);
        // The 5-hour block ends at the reset; if unknown, approximate with the last 5 hours.
        let block_start = reset
            .filter(|r| *r > now)
            .map_or(now.saturating_sub(FIVE_HOURS), |r| r - FIVE_HOURS);
        let tokens = TokensView {
            block_start,
            block: log.totals_since(block_start),
            hourly: log
                .hourly(now)
                .into_iter()
                .map(|(hour, tokens)| Hour { hour, tokens })
                .collect(),
        };
        {
            let state = app.state::<Shared>();
            let mut state = state.lock().unwrap();
            let live: Vec<PaneId> = state.ptys.keys().copied().collect();
            let before = view(&state);
            state.usage.sessions.retain(|p, _| live.contains(p));
            state.usage.tokens = tokens;
            if codex_usage.is_some() {
                state.usage.codex = codex_usage;
            }
            let after = view(&state);
            if before != after {
                let _ = app.emit("usage", after);
            }
        }
        thread::sleep(EVERY);
    }
}
