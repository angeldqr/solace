//! Usage meter: Claude's rate limits arrive from its status line, Codex's and the token totals
//! are read from the files those tools keep.

use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use solace_core::agents::ScreenContext;
use solace_core::terminals::PaneId;
use solace_core::usage::{
    ClaudeSession, ContextUse, Limit, ProviderUsage, Tokens, parse_agent_status,
    parse_claude_status,
};
use solace_infra::usage::{ClaudeLog, CodexCache, CodexContext};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, Shared};

/// Context of Codex panes is polled this often; the heavier reads every `EVERY`.
const TICK: Duration = Duration::from_secs(5);
const EVERY: Duration = Duration::from_secs(30);
const FIVE_HOURS: u64 = 5 * 3600;

#[derive(Default)]
pub struct UsageState {
    claude: Option<ProviderUsage>,
    codex: Option<ProviderUsage>,
    /// From status lines: (agent id, session).
    sessions: HashMap<PaneId, (String, ClaudeSession)>,
    /// Context of the Codex panes read from its rollouts: (agent id, context).
    contexts: HashMap<PaneId, (String, ContextUse)>,
    /// Context an agent prints on its own screen, kept while the agent runs.
    pub(crate) screens: HashMap<PaneId, ScreenContext>,
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
    agent: String,
    model: Option<String>,
    used_tokens: Option<u64>,
    limit: Option<u64>,
    percent: Option<f64>,
    cost_usd: Option<f64>,
}

impl SessionUsage {
    fn new(
        pane: PaneId,
        agent: &str,
        model: Option<String>,
        cost: Option<f64>,
        c: Option<&ContextUse>,
    ) -> Self {
        SessionUsage {
            pane,
            agent: agent.to_string(),
            model: model.or_else(|| c?.model.clone()),
            used_tokens: c.map(|c| c.used),
            limit: c.and_then(|c| c.limit),
            percent: c.and_then(ContextUse::percent),
            cost_usd: cost,
        }
    }

    /// What the agent's screen says wins over the rollout; the window size follows from both
    /// numbers when the footer gives them.
    fn with_screen(mut self, screen: &ScreenContext) -> Self {
        if let Some(used) = screen.used {
            self.used_tokens = Some(used);
        }
        if let Some(percent) = screen.percent {
            self.percent = Some(percent);
            if screen.limit.is_some() {
                self.limit = screen.limit;
            // An integer percent below 10 gives a limit off by up to 50%; only derive it above that.
            } else if let (Some(used), true) = (screen.used, percent >= 10.0) {
                self.limit = Some((used as f64 / (percent / 100.0)).round() as u64);
            }
        }
        self
    }
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
    let claude = state.usage.sessions.iter().map(|(pane, (agent, s))| {
        let (model, cost) = (s.model.clone(), s.cost_usd);
        let session = SessionUsage::new(*pane, agent, model, cost, s.context.as_ref());
        // The status line knows nothing right after /compact; /context on screen fills the gap.
        match (&s.context, state.usage.screens.get(pane)) {
            (None, Some(screen)) => session.with_screen(screen),
            _ => session,
        }
    });
    let mut panes: Vec<PaneId> = state
        .usage
        .contexts
        .keys()
        .chain(state.usage.screens.keys())
        .filter(|p| !state.usage.sessions.contains_key(p))
        .copied()
        .collect();
    panes.sort_by_key(|p| p.0);
    panes.dedup();
    let others = panes.into_iter().filter_map(|pane| {
        let rollout = state.usage.contexts.get(&pane);
        let agent = match rollout {
            Some((agent, _)) => agent,
            None => crate::agents::agent_in(state, pane)?.0,
        };
        let session = SessionUsage::new(pane, agent, None, None, rollout.map(|(_, c)| c));
        Some(match state.usage.screens.get(&pane) {
            Some(screen) => session.with_screen(screen),
            None => session,
        })
    });
    let mut sessions: Vec<SessionUsage> = claude
        .chain(others)
        .filter(|s| state.ptys.contains_key(&s.pane))
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

/// A `solace statusline` report from an agent's pane; only Claude's carries rate limits.
pub fn report(app: &AppHandle, pane: Option<PaneId>, agent: &str, status: &Value) {
    let (limits, session) = if agent == "claude" {
        parse_claude_status(status)
    } else {
        (None, parse_agent_status(agent, status))
    };
    let session = (agent.to_string(), session);
    let state = app.state::<Shared>();
    let mut state = state.lock().unwrap();
    let mut changed = merge(&mut state.usage.claude, limits, None, now_secs() * 1000);
    if let Some(pane) = pane
        && state.usage.sessions.get(&pane) != Some(&session)
    {
        // A /context read from screen is only a stopgap until the status line knows again, and is
        // stale once a /compact empties it.
        let knew = state
            .usage
            .sessions
            .get(&pane)
            .is_some_and(|(_, s)| s.context.is_some());
        if session.1.context.is_some() || knew {
            state.usage.screens.remove(&pane);
        }
        state.usage.sessions.insert(pane, session);
        changed = true;
    }
    if changed {
        let _ = app.emit("usage", view(&state));
    }
}

/// Codex panes, with the folder each works in.
fn context_panes(app: &AppHandle) -> Vec<(PaneId, &'static str, PathBuf)> {
    let state = app.state::<Shared>();
    let state = state.lock().unwrap();
    state
        .agents
        .keys()
        .filter_map(|&pane| {
            let (agent, _) = crate::agents::agent_in(&state, pane)?;
            // Task workers run in their own worktree, not the space's folder.
            let root = crate::tasks::worktree_of(&state, pane)
                .unwrap_or(state.session.pane_root(pane)?.to_path_buf());
            (agent == "codex").then_some((pane, agent, root))
        })
        .collect()
}

pub fn watch(app: AppHandle) {
    let mut log = ClaudeLog::default();
    let mut codex = CodexCache::default();
    let mut codex_ctx = CodexContext::default();
    let codex_home = home_dir("CODEX_HOME", ".codex");
    let projects = home_dir("CLAUDE_CONFIG_DIR", ".claude").map(|d| d.join("projects"));
    let mut heavy: Option<std::time::Instant> = None;
    let mut codex_usage = None;
    let mut tokens = None;
    loop {
        // File reads happen outside the state lock.
        let now = now_secs();
        if heavy.is_none_or(|t| t.elapsed() >= EVERY) {
            heavy = Some(std::time::Instant::now());
            codex_usage = codex_home.as_deref().and_then(|h| codex.read(h));
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
            tokens = Some(TokensView {
                block_start,
                block: log.totals_since(block_start),
                hourly: log
                    .hourly(now)
                    .into_iter()
                    .map(|(hour, tokens)| Hour { hour, tokens })
                    .collect(),
            });
        }
        let contexts: HashMap<PaneId, (String, ContextUse)> = context_panes(&app)
            .into_iter()
            .filter_map(|(pane, agent, root)| {
                let ctx = codex_home
                    .as_deref()
                    .and_then(|h| codex_ctx.read(h, &root))?;
                Some((pane, (agent.to_string(), ctx)))
            })
            .collect();
        {
            let state = app.state::<Shared>();
            let mut state = state.lock().unwrap();
            let live: Vec<PaneId> = state.ptys.keys().copied().collect();
            let before = view(&state);
            state.usage.sessions.retain(|p, _| live.contains(p));
            state.usage.contexts = contexts;
            if let Some(t) = tokens.take() {
                state.usage.tokens = t;
            }
            if codex_usage.is_some() {
                state.usage.codex = codex_usage.take();
            }
            let after = view(&state);
            if before != after {
                let _ = app.emit("usage", after);
            }
        }
        thread::sleep(TICK);
    }
}
