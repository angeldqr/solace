use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use solace_core::session::Session;
use solace_core::spaces::{SpaceId, TabId};
use solace_core::terminals::{Direction, PaneId};
use solace_infra::pty::{Launch, Output, Pty};
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

pub(crate) fn view(session: &Session) -> SessionView {
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
    crate::files::sync_watchers(state);
    if let Err(err) = persist::save(&state.file, &state.session) {
        eprintln!("could not save session: {err}");
    }
    let _ = app.emit("session", view(&state.session));
    // A task whose pane just went away reads as closed.
    if !state.batches.is_empty() {
        let _ = app.emit("tasks", crate::tasks::views(state));
    }
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

/// Agents whose command is on PATH, as (id, name). A WSL shell has its own PATH that can't be
/// checked from here, so every agent is offered then.
fn installed_agents(wsl: bool, on_path: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    solace_core::agents::catalog()
        .iter()
        .filter(|a| wsl || a.command().is_some_and(&on_path))
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect()
}

/// Scanning PATH is slow on Windows, so it runs off the main thread and only once per run (PATH
/// rarely changes while Solace is open).
#[tauri::command(async)]
pub fn agents_installed(state: State<Shared>) -> Vec<(String, String)> {
    static ON_PATH: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    let wsl = state
        .lock()
        .unwrap()
        .settings
        .shell
        .as_ref()
        .is_some_and(|s| s.to_string_lossy().to_lowercase().contains("wsl"));
    if wsl {
        return installed_agents(true, solace_infra::system::on_path);
    }
    ON_PATH
        .get_or_init(|| installed_agents(false, solace_infra::system::on_path))
        .clone()
}

/// Opens a tab and starts the agent in it.
#[tauri::command]
pub fn tab_new_agent(app: AppHandle, state: State<Shared>, space: SpaceId, kind: String) -> Result {
    let def = solace_core::agents::catalog()
        .iter()
        .find(|a| a.id == kind)
        .ok_or_else(|| format!("unknown agent kind {kind:?}"))?;
    let pane = {
        let mut state = state.lock().unwrap();
        let pane = state.session.new_tab(space).ok_or("not found")?;
        if let Err(err) = spawn_shell(&app, &mut state, pane, 120, 32, None, None, &[]) {
            eprintln!("could not start shell: {err}");
        }
        commit(&app, &state);
        pane
    };
    std::thread::spawn(move || {
        if let Err(err) = crate::orchestration::start_command(&app, pane, def, &|_| true) {
            eprintln!("could not start {}: {err}", def.id);
        }
    });
    Ok(())
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
    spawn_shell(&app, &mut state, pane, cols, rows, Some(sink), None, &[])
}

/// Starts the shell for `pane`. The UI usually does this on first sight, but agents can open
/// panes nobody is looking at yet; their output is mirrored and repainted when the UI attaches.
/// `cwd` replaces the space's folder (a task's worktree); `extra_env` adds to the variables
/// every shell gets.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_shell(
    app: &AppHandle,
    state: &mut AppState,
    pane: PaneId,
    cols: u16,
    rows: u16,
    sink: Option<Output>,
    cwd: Option<&Path>,
    extra_env: &[(&str, String)],
) -> Result {
    let cwd = match cwd {
        Some(dir) => dir.to_path_buf(),
        None => state
            .session
            .pane_root(pane)
            .ok_or("unknown pane")?
            .to_path_buf(),
    };
    if !cwd.is_dir() {
        return Err(format!("{} no longer exists", cwd.display()));
    }
    let space = state.session.space_of(pane).ok_or("unknown pane")?;
    let mut env = vec![
        ("SOLACE_ENV", "1".to_string()),
        ("SOLACE_PANE_ID", pane.0.to_string()),
        ("SOLACE_SPACE_ID", space.0.to_string()),
        ("SOLACE_PIPE", crate::endpoint().to_string()),
        // Lets these through to a WSL shell, so `solace.exe` works there too.
        ("WSLENV", wslenv()),
    ];
    env.extend(extra_env.iter().cloned());
    // The `solace` CLI ships in its own folder so the rest of the install dir (the uninstaller,
    // the app itself) stays off the shell's PATH.
    let bin_dir = app.path().resource_dir().ok().map(|dir| dir.join("bin"));
    let handle = app.clone();
    let launch = Launch {
        cwd: &cwd,
        env: &env,
        bin_dir: bin_dir.as_deref(),
        shell: state.settings.shell.as_deref(),
    };
    let pty = Pty::spawn(&launch, cols, rows, sink, move |code| {
        shell_exited(&handle, pane, code)
    })
    .map_err(|e| e.to_string())?;
    state.ptys.insert(pane, pty);
    Ok(())
}

/// The user's WSLENV plus Solace's variables.
fn wslenv() -> String {
    const OURS: &str = "SOLACE_ENV:SOLACE_PANE_ID:SOLACE_SPACE_ID:SOLACE_PIPE:SOLACE_TASK_ID";
    match std::env::var("WSLENV") {
        Ok(existing) if !existing.is_empty() => format!("{existing}:{OURS}"),
        _ => OURS.to_string(),
    }
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

/// False when the pane has no shell (it ended), so the UI can offer to start a new one.
#[tauri::command]
pub fn pty_write(state: State<Shared>, pane: PaneId, data: String) -> bool {
    let state = state.lock().unwrap();
    let pty = state.ptys.get(&pane);
    if let Some(pty) = pty {
        pty.write(data.into_bytes());
    }
    pty.is_some()
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

#[cfg(test)]
mod tests {
    use super::installed_agents;

    #[test]
    fn only_agents_on_path_are_offered_unless_wsl() {
        let found = installed_agents(false, |c| c == "claude");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "claude");
        assert!(installed_agents(false, |_| false).is_empty());
        assert!(installed_agents(true, |_| false).len() > 1);
    }
}
