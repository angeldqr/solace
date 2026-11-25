use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use solace_core::agents::{AgentDef, AgentState, StateTracker};
use solace_core::terminals::PaneId;
use solace_infra::processes::Processes;
use tauri::{AppHandle, Emitter, Manager, UserAttentionType};
use tauri_plugin_notification::NotificationExt;

use crate::notify::{Notice, notice, text};

use crate::{AppState, Shared};

const TICK: Duration = Duration::from_millis(250);
/// The process table is the expensive part, so it is rescanned every few ticks only.
const SCAN_EVERY: u32 = 4;
/// Without a matching screen rule, output streaming this long means the agent is working;
/// shorter bursts are redraws (a resize, a focus change), not work.
const WORKING_AFTER: Duration = Duration::from_secs(1);
/// Minimum gap between two "needs you" notifications for the same agent.
const NEEDS_YOU_AGAIN: Duration = Duration::from_secs(20);

pub struct AgentEntry {
    def: &'static AgentDef,
    tracker: StateTracker,
    /// Unix ms of the last state change, so the UI can show how long it has been.
    since: u64,
    /// When the user was last told this agent needs them: a dialog that flickers while it
    /// redraws must not fire a notification per frame.
    last_needs_you: Option<Instant>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentView {
    pane: PaneId,
    agent: &'static str,
    name: &'static str,
    /// Name given by `solace agent start`, if any.
    label: Option<String>,
    state: AgentState,
    since: u64,
}

/// The agent running in `pane` (catalog id) and its state.
pub fn agent_in(state: &AppState, pane: PaneId) -> Option<(&'static str, AgentState)> {
    let entry = state.agents.get(&pane)?;
    Some((entry.def.id.as_str(), entry.tracker.state))
}

pub fn views(state: &AppState) -> Vec<AgentView> {
    let mut views: Vec<AgentView> = state
        .agents
        .iter()
        .map(|(&pane, entry)| AgentView {
            pane,
            agent: &entry.def.id,
            name: &entry.def.name,
            label: state.names.get(&pane).cloned(),
            state: entry.tracker.state,
            since: entry.since,
        })
        .collect();
    views.sort_by_key(|v| v.pane.0);
    views
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Polls every pane: which agent runs under its shell, and what state its screen shows.
pub fn watch(app: AppHandle) {
    let Some(mut processes) = Processes::new() else {
        return;
    };
    let mut tick = 0u32;
    loop {
        if tick.is_multiple_of(SCAN_EVERY) {
            // Outside the state lock: a full scan can take tens of milliseconds.
            processes.refresh();
        }
        let scanned = tick.is_multiple_of(SCAN_EVERY);
        let state = app.state::<Shared>();
        let mut state = state.lock().unwrap();
        let mut notices = Vec::new();
        if update(&mut state, scanned.then_some(&processes), &mut notices) {
            let _ = app.emit("agents", views(&state));
        }
        let (enabled, locale) = (state.settings.notifications, state.locale.clone());
        drop(state);
        if enabled && !notices.is_empty() {
            deliver(&app, notices, &locale);
        }
        tick = tick.wrapping_add(1);
        thread::sleep(TICK);
    }
}

/// A state change the user may want to hear about, with what's needed to phrase it.
struct Pending {
    notice: Notice,
    agent: &'static str,
    label: Option<String>,
    space: String,
    /// The pane is in the tab on screen; with the window focused, the user already sees it.
    in_view: bool,
}

/// Whether the user is looking at Solace right now.
fn window_in_use(window: &tauri::WebviewWindow) -> bool {
    #[cfg(windows)]
    {
        window
            .hwnd()
            .is_ok_and(|hwnd| solace_infra::system::is_foreground(hwnd.0 as isize))
    }
    #[cfg(not(windows))]
    {
        window.is_focused().unwrap_or(false)
    }
}

/// Native notifications for agents the user isn't looking at; an agent that needs them also
/// flashes the taskbar button until the window comes back to the front.
fn deliver(app: &AppHandle, notices: Vec<Pending>, locale: &str) {
    let window = app.get_webview_window("main");
    let focused = window.as_ref().is_some_and(window_in_use);
    for pending in notices {
        if focused && pending.in_view {
            continue;
        }
        let (title, body) = text(
            &pending.notice,
            pending.agent,
            pending.label.as_deref(),
            &pending.space,
            locale,
        );
        if let Err(err) = app.notification().builder().title(title).body(body).show() {
            eprintln!("could not show a notification: {err}");
        }
        if pending.notice == Notice::NeedsYou
            && !focused
            && let Some(window) = &window
        {
            let _ = window.request_user_attention(Some(UserAttentionType::Critical));
        }
    }
}

/// True when anything the UI shows changed. Agents are re-identified only after a fresh scan;
/// between scans the screen is still read every tick.
fn update(state: &mut AppState, scan: Option<&Processes>, notices: &mut Vec<Pending>) -> bool {
    let AppState {
        ptys,
        agents,
        names,
        session,
        ..
    } = state;
    let before = agents.len();
    agents.retain(|pane, _| ptys.contains_key(pane));
    let mut changed = agents.len() != before;
    let now = Instant::now();

    for (pane, pty) in ptys.iter() {
        let found = match scan {
            Some(processes) => pty.pid().and_then(|pid| processes.find_agent(pid)),
            None => agents.get(pane).map(|entry| entry.def),
        };
        let Some(def) = found else {
            if agents.remove(pane).is_some() {
                // The named agent quit; the name must not follow whatever runs there next.
                names.remove(pane);
                changed = true;
            }
            continue;
        };
        let entry = agents.entry(*pane).or_insert_with(|| {
            changed = true;
            AgentEntry {
                def,
                tracker: StateTracker::default(),
                since: unix_ms(),
                last_needs_you: None,
            }
        });
        if entry.def.id != def.id {
            names.remove(pane);
            *entry = AgentEntry {
                def,
                tracker: StateTracker::default(),
                since: unix_ms(),
                last_needs_you: None,
            };
            changed = true;
        }
        let screen = pty.snapshot();
        let streaming = pty.busy_for().is_some_and(|busy| busy >= WORKING_AFTER);
        let fallback = if streaming {
            AgentState::Working
        } else {
            AgentState::Idle
        };
        let verdict = def.detect(&screen.title, &screen.rows).unwrap_or(fallback);
        let (before, since) = (entry.tracker.state, entry.since);
        if entry.tracker.observe(verdict, now) {
            entry.since = unix_ms();
            changed = true;
            let worked = Duration::from_millis(entry.since.saturating_sub(since));
            let notice =
                notice(before, entry.tracker.state, worked).filter(|notice| match notice {
                    Notice::NeedsYou => entry
                        .last_needs_you
                        .is_none_or(|at| now.duration_since(at) >= NEEDS_YOU_AGAIN),
                    // Without screen rules, a pause mid-task looks like "finished": don't claim it.
                    Notice::Finished => def.has_rules(),
                });
            if notice == Some(Notice::NeedsYou) {
                entry.last_needs_you = Some(now);
            }
            if let Some(notice) = notice {
                let space = session
                    .spaces
                    .iter()
                    .find(|s| s.tabs.iter().any(|t| t.layout.contains(*pane)));
                let in_view = session.active_space == space.map(|s| s.id)
                    && space.is_some_and(|s| {
                        s.tabs
                            .iter()
                            .any(|t| Some(t.id) == s.active_tab && t.layout.contains(*pane))
                    });
                notices.push(Pending {
                    notice,
                    agent: &def.name,
                    label: names.get(pane).cloned(),
                    space: space.map_or_else(|| "Solace".to_string(), |s| s.name.clone()),
                    in_view,
                });
            }
        }
    }
    changed
}
