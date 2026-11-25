use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use solace_core::session::Session;
use solace_core::spaces::{SpaceId, TabId};
use solace_core::terminals::{Direction, PaneId};
use solace_infra::pty::{Output, Pty};
use solace_infra::{git, persist};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, Shared};

type Result<T = ()> = std::result::Result<T, String>;

/// What the UI renders: the session plus facts read from disk.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    #[serde(flatten)]
    session: Session,
    branches: HashMap<SpaceId, String>,
}

fn view(session: &Session) -> SessionView {
    let branches = session
        .spaces
        .iter()
        .filter_map(|s| Some((s.id, git::current_branch(&s.root)?)))
        .collect();
    SessionView {
        session: session.clone(),
        branches,
    }
}

/// Persists and broadcasts the session after every change.
pub(crate) fn commit(app: &AppHandle, state: &AppState) {
    if let Err(err) = persist::save(&state.file, &state.session) {
        eprintln!("could not save session: {err}");
    }
    let _ = app.emit("session", view(&state.session));
}

/// Runs a use case; `None` means an unknown id. Panes it reports as closed get their shells killed.
fn update(
    app: &AppHandle,
    state: &Shared,
    use_case: impl FnOnce(&mut Session) -> Option<Vec<PaneId>>,
) -> Result {
    let mut state = state.lock().unwrap();
    let closed = use_case(&mut state.session).ok_or("not found")?;
    for pane in closed {
        if let Some(pty) = state.ptys.remove(&pane) {
            pty.kill();
        }
    }
    commit(app, &state);
    Ok(())
}

fn none_closed(ok: bool) -> Option<Vec<PaneId>> {
    ok.then(Vec::new)
}

#[tauri::command]
pub fn agents_get(state: State<Shared>) -> Vec<crate::agents::AgentView> {
    crate::agents::views(&state.lock().unwrap())
}

/// Called once per page load. Panes the reloaded page doesn't show yet would otherwise stay
/// paused with nobody left to resume them, silently stalling their shells.
#[tauri::command]
pub fn session_get(state: State<Shared>) -> SessionView {
    let state = state.lock().unwrap();
    for pty in state.ptys.values() {
        pty.set_paused(false);
    }
    view(&state.session)
}

#[tauri::command]
pub fn space_open(app: AppHandle, state: State<Shared>, path: String) -> Result {
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    update(&app, &state, |s| {
        s.open_space(root);
        Some(Vec::new())
    })
}

#[tauri::command]
pub fn space_close(app: AppHandle, state: State<Shared>, space: SpaceId) -> Result {
    update(&app, &state, |s| s.close_space(space))
}

#[tauri::command]
pub fn space_activate(app: AppHandle, state: State<Shared>, space: SpaceId) -> Result {
    update(&app, &state, |s| none_closed(s.activate_space(space)))
}

#[tauri::command]
pub fn tab_new(app: AppHandle, state: State<Shared>, space: SpaceId) -> Result {
    update(&app, &state, |s| s.new_tab(space).map(|_| Vec::new()))
}

#[tauri::command]
pub fn tab_activate(app: AppHandle, state: State<Shared>, tab: TabId) -> Result {
    update(&app, &state, |s| none_closed(s.activate_tab(tab)))
}

#[tauri::command]
pub fn tab_close(app: AppHandle, state: State<Shared>, tab: TabId) -> Result {
    update(&app, &state, |s| s.close_tab(tab))
}

#[tauri::command]
pub fn pane_split(
    app: AppHandle,
    state: State<Shared>,
    pane: PaneId,
    direction: Direction,
) -> Result {
    update(&app, &state, |s| {
        s.split_pane(pane, direction).map(|_| Vec::new())
    })
}

#[tauri::command]
pub fn pane_close(app: AppHandle, state: State<Shared>, pane: PaneId) -> Result {
    update(&app, &state, |s| s.close_pane(pane).then(|| vec![pane]))
}

#[tauri::command]
pub fn pane_focus(app: AppHandle, state: State<Shared>, pane: PaneId) -> Result {
    update(&app, &state, |s| none_closed(s.focus_pane(pane)))
}

#[tauri::command]
pub fn split_resize(
    app: AppHandle,
    state: State<Shared>,
    tab: TabId,
    path: Vec<u8>,
    ratio: f32,
) -> Result {
    update(&app, &state, |s| {
        none_closed(s.resize_split(tab, &path, ratio))
    })
}

/// Connects a pane's terminal to its shell, starting the shell on first attach.
/// Re-attaching (e.g. after a UI reload) only redirects output.
/// Async so starting a shell never stalls the UI thread.
#[tauri::command(async)]
pub fn pty_attach(
    app: AppHandle,
    state: State<Shared>,
    pane: PaneId,
    cols: u16,
    rows: u16,
    output: Channel<InvokeResponseBody>,
) -> Result {
    let mut state = state.lock().unwrap();
    // Raw bytes reach JS as an ArrayBuffer; a Vec<u8> would be sent as a JSON array.
    let sink: Output = Box::new(move |bytes| {
        let _ = output.send(InvokeResponseBody::Raw(bytes));
    });
    if let Some(pty) = state.ptys.get(&pane) {
        pty.set_output(sink);
        pty.set_paused(false);
        return pty.resize(cols, rows).map_err(|e| e.to_string());
    }
    spawn_shell(&app, &mut state, pane, cols, rows, Some(sink))
}

/// Starts the shell for `pane`. The UI usually does this on first sight, but agents can open
/// panes nobody is looking at yet; their output is mirrored and repainted when the UI attaches.
pub(crate) fn spawn_shell(
    app: &AppHandle,
    state: &mut AppState,
    pane: PaneId,
    cols: u16,
    rows: u16,
    sink: Option<Output>,
) -> Result {
    let cwd = state
        .session
        .pane_root(pane)
        .ok_or("unknown pane")?
        .to_path_buf();
    if !cwd.is_dir() {
        return Err(format!("{} no longer exists", cwd.display()));
    }
    let space = state.session.space_of(pane).ok_or("unknown pane")?;
    let env = [
        ("SOLACE_ENV", "1".to_string()),
        ("SOLACE_PANE_ID", pane.0.to_string()),
        ("SOLACE_SPACE_ID", space.0.to_string()),
        ("SOLACE_PIPE", crate::endpoint().to_string()),
    ];
    // The `solace` CLI ships in its own folder so the rest of the install dir (the uninstaller,
    // the app itself) stays off the shell's PATH.
    let bin_dir = app.path().resource_dir().ok().map(|dir| dir.join("bin"));
    let handle = app.clone();
    let pty = Pty::spawn(
        &cwd,
        &env,
        bin_dir.as_deref(),
        cols,
        rows,
        sink,
        move |code| shell_exited(&handle, pane, code),
    )
    .map_err(|e| e.to_string())?;
    state.ptys.insert(pane, pty);
    Ok(())
}

/// A shell ended on its own. If the pty is already gone, the pane was closed by the user or the
/// app is shutting down, and the session must stay as it is. Like Windows Terminal, only a clean
/// `exit` closes the pane; a crash or an external kill (e.g. logoff) keeps it, showing the code.
fn shell_exited(app: &AppHandle, pane: PaneId, code: Option<u32>) {
    let state = app.state::<Shared>();
    let mut state = state.lock().unwrap();
    if state.ptys.remove(&pane).is_some() && code == Some(0) && state.session.close_pane(pane) {
        commit(app, &state);
    }
}

#[tauri::command]
pub fn pty_write(state: State<Shared>, pane: PaneId, data: String) {
    if let Some(pty) = state.lock().unwrap().ptys.get(&pane) {
        pty.write(data.into_bytes());
    }
}

#[tauri::command]
pub fn pty_resize(state: State<Shared>, pane: PaneId, cols: u16, rows: u16) -> Result {
    match state.lock().unwrap().ptys.get(&pane) {
        Some(pty) => pty.resize(cols, rows).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn pty_pause(state: State<Shared>, pane: PaneId, paused: bool) {
    if let Some(pty) = state.lock().unwrap().ptys.get(&pane) {
        pty.set_paused(paused);
    }
}
