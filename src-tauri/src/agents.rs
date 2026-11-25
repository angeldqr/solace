use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use solace_core::agents::{AgentDef, AgentState, StateTracker};
use solace_core::terminals::PaneId;
use solace_infra::processes::Processes;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, Shared};

const TICK: Duration = Duration::from_millis(250);
/// The process table is the expensive part, so it is rescanned every few ticks only.
const SCAN_EVERY: u32 = 4;
/// Without a matching screen rule, output streaming this long means the agent is working;
/// shorter bursts are redraws (a resize, a focus change), not work.
const WORKING_AFTER: Duration = Duration::from_secs(1);

pub struct AgentEntry {
    def: &'static AgentDef,
    tracker: StateTracker,
    /// Unix ms of the last state change, so the UI can show how long it has been.
    since: u64,
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
        if update(&mut state, scanned.then_some(&processes)) {
            let _ = app.emit("agents", views(&state));
        }
        drop(state);
        tick = tick.wrapping_add(1);
        thread::sleep(TICK);
    }
}

/// True when anything the UI shows changed. Agents are re-identified only after a fresh scan;
/// between scans the screen is still read every tick.
fn update(state: &mut AppState, scan: Option<&Processes>) -> bool {
    let AppState {
        ptys,
        agents,
        names,
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
            }
        });
        if entry.def.id != def.id {
            names.remove(pane);
            *entry = AgentEntry {
                def,
                tracker: StateTracker::default(),
                since: unix_ms(),
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
        if entry.tracker.observe(verdict, now) {
            entry.since = unix_ms();
            changed = true;
        }
    }
    changed
}
