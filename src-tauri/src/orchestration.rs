use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use solace_core::agents::{AgentDef, AgentState, catalog};
use solace_core::orchestration::{Envelope, Request, Response};
use solace_core::terminals::{Direction, PaneId};
use tauri::{AppHandle, Manager};

use crate::agents::agent_in;
use crate::commands::{commit, spawn_shell};
use crate::{AppState, Shared};

const POLL: Duration = Duration::from_millis(200);
/// Panes opened by agents get a shell at once; the UI resizes it when it shows the pane.
const INITIAL_SIZE: (u16, u16) = (120, 32);
const SHELL_READY_WITHIN: Duration = Duration::from_secs(10);
const AGENT_READY_WITHIN: Duration = Duration::from_secs(60);
const PICKUP_WITHIN: Duration = Duration::from_secs(20);
/// Caps caller-supplied timeouts so a huge value can't overflow the deadline.
const MAX_WAIT: Duration = Duration::from_secs(24 * 60 * 60);

type Result<T = Value> = std::result::Result<T, String>;

/// The pane behind a `SOLACE_PANE_TOKEN`, and for a task's worker, its task.
#[derive(Clone)]
pub(crate) struct Caller {
    pub pane: PaneId,
    pub task: Option<String>,
}

/// Every request must carry its pane's token; the pane it names is the only identity trusted.
pub fn handle(app: &AppHandle, envelope: Envelope) -> Response {
    let caller = with_state(app, |s| {
        let caller = s.callers.get(&envelope.token)?;
        s.session.pane_root(caller.pane)?;
        Some(caller.clone())
    })
    .ok_or("not allowed: SOLACE_PANE_TOKEN is missing or belongs to a closed pane")?;
    let request = bind(envelope.request, caller.pane)?;
    authorize(&request, &caller)?;
    dispatch(app, request, &caller)
}

/// Makes the pane ids a request names its caller's: a CLI that claims another pane is refused.
fn bind(mut request: Request, pane: PaneId) -> Result<Request> {
    fn claim(slot: &mut Option<PaneId>, pane: PaneId) -> Result<()> {
        match slot {
            Some(named) if *named != pane => Err(format!(
                "SOLACE_PANE_ID ({}) does not match this pane",
                named.0
            )),
            _ => {
                *slot = Some(pane);
                Ok(())
            }
        }
    }
    match &mut request {
        Request::AgentStart { from, .. }
        | Request::TasksStart { from, .. }
        | Request::TasksWait { from, .. }
        | Request::TasksClean { from, .. } => claim(from, pane)?,
        Request::TaskDone { pane: named, .. } | Request::UsageReport { pane: named, .. } => {
            claim(named, pane)?
        }
        Request::AgentReport { pane: named, .. } if *named != pane => {
            return Err(format!(
                "SOLACE_PANE_ID ({}) does not match this pane",
                named.0
            ));
        }
        _ => {}
    }
    Ok(request)
}

/// A task's worker may report and look, nothing more: it must not drive the user's panes or
/// start agents. `task.done` is only for its own task.
fn authorize(request: &Request, caller: &Caller) -> Result<()> {
    let Some(own) = &caller.task else {
        return Ok(());
    };
    match request {
        Request::TaskDone { task, .. } if task != own => {
            Err(format!("you may only finish your own task ({own})"))
        }
        Request::TaskDone { .. }
        | Request::AgentReport { .. }
        | Request::UsageReport { .. }
        | Request::PaneList
        | Request::AgentList
        | Request::TasksWait { .. } => Ok(()),
        _ => {
            Err("a task's worker may only report its task and list panes, agents and tasks".into())
        }
    }
}

/// Typing goes to agents, or to panes the caller opened; never into the user's own shells.
fn may_type(target_has_agent: bool, target_creator: Option<PaneId>, caller: PaneId) -> bool {
    target_has_agent || target_creator == Some(caller)
}

fn check_typing(app: &AppHandle, target: PaneId, caller: &Caller) -> Result<()> {
    let allowed = with_state(app, |s| {
        may_type(
            agent_in(s, target).is_some(),
            s.creators.get(&target).copied(),
            caller.pane,
        )
    });
    if allowed {
        Ok(())
    } else {
        Err(
            "that pane runs no agent and was not opened by you; only the user may type there"
                .into(),
        )
    }
}

/// Remembers who opened `reply["pane"]`, so that caller may type into it later.
fn opened_by(app: &AppHandle, caller: &Caller, reply: Value) -> Value {
    if let Some(pane) = reply.get("pane").and_then(Value::as_u64) {
        with_state(app, |s| s.creators.insert(PaneId(pane as u32), caller.pane));
    }
    reply
}

fn dispatch(app: &AppHandle, request: Request, caller: &Caller) -> Response {
    match request {
        Request::PaneList => Ok(with_state(app, pane_list)),
        Request::PaneSplit { pane, direction } => split(app, pane, direction, None, None, &[])
            .map(|pane| opened_by(app, caller, json!({ "pane": pane }))),
        Request::PaneSend {
            target,
            text,
            enter,
        } => {
            let pane = resolve(app, &target)?;
            check_typing(app, pane, caller)?;
            type_text(app, pane, &text, enter, false)?;
            Ok(json!({ "pane": pane }))
        }
        Request::PaneRead { target, lines } => {
            let pane = resolve(app, &target)?;
            Ok(json!({ "pane": pane, "screen": screen(app, pane, lines)? }))
        }
        Request::PaneClose { target } => {
            let pane = resolve(app, &target)?;
            close(app, pane);
            Ok(json!({ "closed": pane }))
        }
        Request::AgentStart {
            name,
            kind,
            from,
            direction,
            prompt,
        } => start_agent(app, name, &kind, from, direction, prompt)
            .map(|reply| opened_by(app, caller, reply)),
        Request::AgentPrompt {
            target,
            text,
            wait,
            timeout_secs,
        } => {
            let pane = resolve(app, &target)?;
            check_typing(app, pane, caller)?;
            send_prompt(
                app,
                &target,
                &text,
                wait,
                Duration::from_secs(timeout_secs).min(MAX_WAIT),
            )
        }
        Request::AgentWait {
            target,
            timeout_secs,
        } => {
            let pane = resolve(app, &target)?;
            settle(app, pane, Duration::from_secs(timeout_secs).min(MAX_WAIT))
        }
        Request::AgentList => Ok(json!(with_state(app, |s| crate::agents::views(s)))),
        Request::AgentReport {
            pane,
            session,
            state,
        } => {
            with_state(app, |s| {
                if s.reports.get(&pane).is_some_and(|r| r.session != session) {
                    return; // another Claude inside this pane's agent, e.g. `claude -p` from a tool
                }
                match state {
                    Some(state) => {
                        let at = Instant::now();
                        s.reports
                            .insert(pane, crate::agents::Report { state, session, at });
                    }
                    None => {
                        s.reports.remove(&pane);
                    }
                }
            });
            Ok(json!({ "pane": pane }))
        }
        Request::TasksStart {
            from,
            cwd,
            kind,
            shared,
            tasks,
        } => crate::tasks::start(app, from, cwd, kind, shared, tasks),
        Request::TasksWait {
            batch,
            from,
            timeout_secs,
        } => crate::tasks::wait(
            app,
            batch,
            from,
            Duration::from_secs(timeout_secs).min(MAX_WAIT),
        ),
        Request::TasksClean { batch, force, from } => crate::tasks::clean(app, &batch, force, from),
        Request::TaskDone {
            task,
            pane,
            summary,
            failed,
            no_changes,
        } => crate::tasks::done(app, &task, pane, &summary, failed, no_changes),
        Request::UsageReport { pane, status } => {
            crate::usage::report(app, pane, &status);
            Ok(json!({}))
        }
    }
}

pub(crate) fn with_state<T>(app: &AppHandle, f: impl FnOnce(&mut AppState) -> T) -> T {
    let state = app.state::<Shared>();
    let mut state = state.lock().unwrap();
    f(&mut state)
}

/// Polls `done` without holding the state lock in between; false on timeout.
pub(crate) fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL);
    }
}

/// A pane id, or the name an agent was started with.
fn resolve(app: &AppHandle, target: &str) -> Result<PaneId> {
    with_state(app, |s| {
        if let Ok(id) = target.parse::<u32>()
            && s.session.pane_root(PaneId(id)).is_some()
        {
            return Ok(PaneId(id));
        }
        s.names
            .iter()
            .find(|(pane, name)| *name == target && s.session.pane_root(**pane).is_some())
            .map(|(pane, _)| *pane)
            .ok_or_else(|| format!("no pane or agent named {target:?}"))
    })
}

/// Opens a pane beside `from` (default: the focused one) with a shell already running. Focus and
/// the visible tab never move: the user may be typing elsewhere, and a stolen Enter could
/// confirm something in the agent's pane. A `name` is reserved in the same step, so two
/// concurrent starts can't both claim it.
/// `cwd` and `env` start the shell elsewhere and with more variables (see `spawn_shell`).
pub(crate) fn split(
    app: &AppHandle,
    from: Option<PaneId>,
    direction: Direction,
    name: Option<&str>,
    cwd: Option<&Path>,
    env: &[(&str, String)],
) -> Result<PaneId> {
    with_state(app, |s| {
        if let Some(name) = name {
            if name.is_empty() || name.parse::<u32>().is_ok() {
                return Err("agent names must contain a letter (numbers are pane ids)".into());
            }
            let session = &s.session;
            s.names.retain(|pane, _| session.pane_root(*pane).is_some());
            if s.names.values().any(|n| n == name) {
                return Err(format!("an agent named {name:?} already exists"));
            }
        }
        let from = from.or_else(|| focused_pane(s)).ok_or("no pane to split")?;
        let pane = s
            .session
            .split_pane_unfocused(from, direction)
            .ok_or("unknown pane")?;
        let (cols, rows) = INITIAL_SIZE;
        spawn_shell(app, s, pane, cols, rows, None, cwd, env)?;
        if let Some(name) = name {
            s.names.insert(pane, name.to_string());
        }
        commit(app, s);
        Ok(pane)
    })
}

/// The pane the user is looking at.
pub(crate) fn focused_pane(s: &AppState) -> Option<PaneId> {
    let space = s
        .session
        .spaces
        .iter()
        .find(|sp| Some(sp.id) == s.session.active_space)?;
    let tab = space.tabs.iter().find(|t| Some(t.id) == space.active_tab)?;
    Some(tab.focused)
}

pub(crate) fn close(app: &AppHandle, pane: PaneId) {
    with_state(app, |s| {
        if s.session.close_pane(pane) {
            if let Some(pty) = s.ptys.remove(&pane) {
                pty.kill();
            }
            s.names.remove(&pane);
            s.creators.remove(&pane);
            s.callers.retain(|_, c| c.pane != pane);
            commit(app, s);
        }
    });
}

/// Types into a pane. Control characters are dropped so text can never act as keystrokes (an
/// embedded ESC[201~ would end a paste early, ESC alone cancels in most agents). Prompts go as a
/// bracketed paste when the program supports it, so multi-line text arrives as one message, with
/// Enter sent separately afterwards: agents treat an Enter inside a paste as a newline.
fn type_text(app: &AppHandle, pane: PaneId, text: &str, enter: bool, paste: bool) -> Result<()> {
    let text = printable(text);
    let written = with_state(app, |s| {
        let pty = s.ptys.get(&pane)?;
        let bytes = if paste && pty.bracketed_paste() {
            format!("\x1b[200~{text}\x1b[201~")
        } else {
            text
        };
        pty.write(bytes.into_bytes());
        Some(())
    });
    written.ok_or("the pane has no running shell")?;
    if enter {
        thread::sleep(Duration::from_millis(60));
        with_state(app, |s| {
            s.ptys.get(&pane).map(|pty| pty.write(b"\r".to_vec()))
        });
    }
    Ok(())
}

/// Text with control characters removed, keeping line breaks and tabs.
fn printable(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

fn screen(app: &AppHandle, pane: PaneId, lines: Option<usize>) -> Result<String> {
    let rows = with_state(app, |s| s.ptys.get(&pane).map(|pty| pty.snapshot().rows))
        .ok_or("the pane has no running shell")?;
    let mut rows: Vec<&str> = rows.iter().map(|r| r.trim_end()).collect();
    while rows.last().is_some_and(|r| r.is_empty()) {
        rows.pop();
    }
    if let Some(n) = lines {
        rows.retain(|r| !r.is_empty());
        rows = rows.split_off(rows.len().saturating_sub(n));
    }
    Ok(rows.join("\n"))
}

fn quiet(app: &AppHandle, pane: PaneId) -> bool {
    with_state(app, |s| {
        s.ptys
            .get(&pane)
            .is_some_and(|pty| pty.busy_for().is_none())
    })
}

fn start_agent(
    app: &AppHandle,
    name: String,
    kind: &str,
    from: Option<PaneId>,
    direction: Direction,
    prompt: Option<String>,
) -> Result {
    let def = catalog()
        .iter()
        .find(|a| a.id == kind)
        .ok_or_else(|| format!("unknown agent kind {kind:?}"))?;
    def.command()
        .ok_or("this agent has no command to start it")?;
    let pane = split(app, from, direction, Some(&name), None, &[])?;
    let mut reply = json!({ "pane": pane, "name": name, "kind": def.id });
    let Some(text) = prompt else {
        start_command(app, pane, def, &|_| true)?;
        return Ok(reply);
    };
    match launch(app, pane, def, &text, AGENT_READY_WITHIN, false, &|_| true)? {
        Launched::Sent => {}
        Launched::Blocked => {
            reply["warning"] = json!(
                "the agent is waiting for the user (e.g. to trust the folder); prompt not sent"
            );
        }
        Launched::TimedOut | Launched::Gone => {
            reply["warning"] = json!("the agent did not become ready in time; prompt not sent");
        }
    }
    Ok(reply)
}

/// Types the agent's command into the pane's shell, once the shell has shown its prompt: typed
/// before that, the command can be swallowed by shell startup.
pub(crate) fn start_command(
    app: &AppHandle,
    pane: PaneId,
    def: &AgentDef,
    live: &dyn Fn(&AppState) -> bool,
) -> Result<()> {
    let command = def
        .command()
        .ok_or("this agent has no command to start it")?;
    let mut seen_output = false;
    wait_until(SHELL_READY_WITHIN, || {
        let busy = !quiet(app, pane);
        seen_output |= busy;
        seen_output && !busy
    });
    // The pane may have been closed, or its task finished or cleaned, while the shell started.
    if !with_state(app, |s| s.session.pane_root(pane).is_some() && live(s)) {
        return Ok(());
    }
    type_text(app, pane, command, true, false)
}

/// How `launch` ended.
pub(crate) enum Launched {
    Sent,
    /// The agent is waiting for the user (e.g. to trust the folder); nothing was sent.
    Blocked,
    TimedOut,
    /// The pane was closed while waiting.
    Gone,
}

/// Starts `def` in the pane and sends it `prompt` once it is ready. Ready means recognized,
/// done drawing and idle, judged by a screen rule (Claude's title) or, for agents without one,
/// by output after detection. A tracker that has seen nothing yet also reads idle, and typeahead
/// landing on a startup dialog (folder trust) would answer it. With `wait_out_blocked` a dialog
/// is waited out (up to `within`) and the prompt goes once the user has answered it. `live`
/// says whether the caller still wants the launch; once it turns false nothing more is typed.
pub(crate) fn launch(
    app: &AppHandle,
    pane: PaneId,
    def: &AgentDef,
    prompt: &str,
    within: Duration,
    wait_out_blocked: bool,
    live: &dyn Fn(&AppState) -> bool,
) -> Result<Launched> {
    start_command(app, pane, def, live)?;
    let (mut detected, mut drew, mut blocked, mut gone) = (false, false, false, false);
    let ready = wait_until(within, || {
        gone = with_state(app, |s| s.session.pane_root(pane).is_none() || !live(s));
        if gone {
            return true;
        }
        let Some((id, state)) = with_state(app, |s| agent_in(s, pane)) else {
            return false;
        };
        if id != def.id {
            return false;
        }
        if state == AgentState::Blocked {
            blocked = !wait_out_blocked;
            return blocked;
        }
        let busy = !quiet(app, pane);
        drew |= detected && busy;
        detected = true;
        let idle_on_screen = with_state(app, |s| {
            s.ptys
                .get(&pane)
                .map(|pty| pty.snapshot())
                .is_some_and(|screen| {
                    def.detect(&screen.title, &screen.rows) == Some(AgentState::Idle)
                })
        });
        state == AgentState::Idle && !busy && (idle_on_screen || drew)
    });
    Ok(if gone {
        Launched::Gone
    } else if blocked {
        Launched::Blocked
    } else if ready {
        // Re-checked right before typing: nothing may be typed into a pane that moved on.
        if !with_state(app, |s| s.session.pane_root(pane).is_some() && live(s)) {
            return Ok(Launched::Gone);
        }
        type_text(app, pane, prompt, true, true)?;
        Launched::Sent
    } else {
        Launched::TimedOut
    })
}

fn send_prompt(app: &AppHandle, target: &str, text: &str, wait: bool, timeout: Duration) -> Result {
    let pane = resolve(app, target)?;
    match with_state(app, |s| agent_in(s, pane)) {
        None => return Err(format!("no agent is running in {target:?}")),
        // Text plus Enter would answer the dialog, usually with its default "Yes".
        Some((_, AgentState::Blocked)) => {
            return Err(format!(
                "{target:?} is waiting for the user (a permission or a question); ask the user to answer it in pane {}",
                pane.0
            ));
        }
        Some(_) => {}
    }
    type_text(app, pane, text, true, true)?;
    if !wait {
        return Ok(json!({ "pane": pane, "sent": true }));
    }
    // A very quick answer can finish before a poll sees it working; then settle returns at once.
    wait_until(PICKUP_WITHIN, || {
        with_state(app, |s| agent_in(s, pane)).is_some_and(|(_, state)| state != AgentState::Idle)
    });
    settle(app, pane, timeout)
}

/// Waits until the agent is idle or needs the user, then reports its state and screen.
fn settle(app: &AppHandle, pane: PaneId, timeout: Duration) -> Result {
    let finished = wait_until(timeout, || {
        with_state(app, |s| agent_in(s, pane)).is_none_or(|(_, state)| state != AgentState::Working)
    });
    let state = with_state(app, |s| agent_in(s, pane)).map(|(_, state)| state);
    Ok(json!({
        "pane": pane,
        "state": state,
        "timedOut": !finished,
        "screen": screen(app, pane, None)?,
    }))
}

fn pane_list(s: &mut AppState) -> Value {
    let mut panes = Vec::new();
    for space in &s.session.spaces {
        for tab in &space.tabs {
            for pane in tab.layout.panes() {
                let agent = agent_in(s, pane);
                panes.push(json!({
                    "pane": pane,
                    "space": space.name,
                    "tab": tab.id,
                    "focused": tab.focused == pane,
                    "agent": agent.map(|(id, _)| id),
                    "state": agent.map(|(_, state)| state),
                    "name": s.names.get(&pane),
                }));
            }
        }
    }
    json!(panes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker(task: &str) -> Caller {
        Caller {
            pane: PaneId(2),
            task: Some(task.into()),
        }
    }

    fn done(task: &str) -> Request {
        Request::TaskDone {
            task: task.into(),
            pane: None,
            summary: String::new(),
            failed: false,
            no_changes: false,
        }
    }

    #[test]
    fn workers_may_only_report_and_look() {
        let lead = Caller {
            pane: PaneId(1),
            task: None,
        };
        let send = || Request::PaneSend {
            target: "1".into(),
            text: "x".into(),
            enter: true,
        };
        assert!(authorize(&send(), &lead).is_ok());
        assert!(authorize(&send(), &worker("b1-1")).is_err());
        assert!(authorize(&Request::PaneList, &worker("b1-1")).is_ok());
        assert!(authorize(&done("b1-1"), &worker("b1-1")).is_ok());
        assert!(authorize(&done("b1-2"), &worker("b1-1")).is_err());
    }

    #[test]
    fn requests_are_bound_to_the_token_pane() {
        let list = |from| Request::TasksWait {
            batch: None,
            from,
            timeout_secs: 0,
        };
        let Ok(Request::TasksWait { from, .. }) = bind(list(None), PaneId(4)) else {
            panic!("refused");
        };
        assert_eq!(from, Some(PaneId(4)));
        assert!(bind(list(Some(PaneId(5))), PaneId(4)).is_err());
    }

    #[test]
    fn typing_needs_an_agent_or_being_the_creator() {
        let me = PaneId(1);
        assert!(may_type(true, None, me));
        assert!(may_type(false, Some(me), me));
        assert!(!may_type(false, None, me));
        assert!(!may_type(false, Some(PaneId(9)), me));
    }

    #[test]
    fn prompts_cannot_smuggle_keystrokes() {
        let hostile = "review this\x1b[201~\x1b[Z\rrm -rf /\x03\nline two\tok";
        assert_eq!(
            printable(hostile),
            "review this[201~[Zrm -rf /\nline two\tok"
        );
    }
}
