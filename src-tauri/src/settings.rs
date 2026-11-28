use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use solace_infra::persist;
use solace_infra::pty::{Shell, available_shells};
use tauri::State;

use crate::Shared;

/// Settings the backend needs; look-and-feel settings live in the UI.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Shell for new terminals; the platform default when unset.
    #[serde(default)]
    pub shell: Option<PathBuf>,
    /// Notify when an agent the user isn't watching needs them or finishes.
    #[serde(default = "on")]
    pub notifications: bool,
}

fn on() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            shell: None,
            notifications: true,
        }
    }
}

/// Async: the PATH scan can be slow (network drives) and must not freeze the window.
#[tauri::command(async)]
pub fn shells_list() -> Vec<Shell> {
    available_shells()
}

#[tauri::command]
pub fn settings_get(state: State<Shared>) -> AppSettings {
    state.lock().unwrap().settings.clone()
}

/// Only a shell found on this machine is accepted: the webview must not choose arbitrary programs.
#[tauri::command(async)]
pub fn shell_set(state: State<Shared>, path: Option<String>) -> Result<(), String> {
    let shell = match path {
        None => None,
        Some(path) => {
            let path = PathBuf::from(path);
            if !available_shells().iter().any(|s| s.path == path) {
                return Err(format!("{} is not an installed shell", path.display()));
            }
            Some(path)
        }
    };
    let mut state = state.lock().unwrap();
    state.settings.shell = shell;
    persist::save(&state.settings_file, &state.settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notifications_set(state: State<Shared>, enabled: bool) -> Result<(), String> {
    let mut state = state.lock().unwrap();
    state.settings.notifications = enabled;
    persist::save(&state.settings_file, &state.settings).map_err(|e| e.to_string())
}

/// The UI's language, so notifications sent from here match it.
#[tauri::command]
pub fn locale_set(state: State<Shared>, locale: String) {
    if locale == "es" || locale == "en" {
        state.lock().unwrap().locale = locale;
    }
}

/// False when the OS has notifications switched off, so the UI can say why none appear.
#[tauri::command]
pub fn system_toasts_enabled() -> bool {
    solace_infra::system::toasts_enabled()
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "no home directory".into())
}

/// Opens the folder holding Solace's log file in the file manager.
#[tauri::command]
pub fn open_log_dir(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Which agent CLIs have Solace's integration installed, by catalog id.
#[tauri::command(async)]
pub fn integrations_installed() -> Result<Vec<String>, String> {
    let home = home()?;
    Ok(solace_infra::integrations::AGENTS
        .iter()
        .filter(|a| solace_infra::integrations::installed(a, &home))
        .map(|a| a.to_string())
        .collect())
}

/// The `solace` CLI shipped with this app; hooks point at it so they survive updates.
fn bundled_cli(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("bin").join(if cfg!(windows) {
        "solace.exe"
    } else {
        "solace"
    }))
}

/// The `solace` CLI the hooks run. An AppImage's files live in a mount that vanishes when the app
/// quits, under a /tmp name other users can see and recreate, so there the CLI is copied first to
/// a folder only this user can write to.
fn hook_cli(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let cli = bundled_cli(app)?;
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        use tauri::Manager;
        let dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| e.to_string())?
            .join("bin");
        let err = |e: std::io::Error| e.to_string();
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(err)?;
        let (copy, partial) = (dir.join("solace"), dir.join("solace.partial"));
        std::fs::copy(&cli, &partial).map_err(err)?;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o700)).map_err(err)?;
        // A rename replaces the old copy even while a hook is running it.
        std::fs::rename(&partial, &copy).map_err(err)?;
        return Ok(copy);
    }
    Ok(cli)
}

#[tauri::command(async)]
pub fn integration_set(app: tauri::AppHandle, agent: String, enabled: bool) -> Result<(), String> {
    let home = home()?;
    let result = if enabled {
        solace_infra::integrations::install(&agent, &home, &hook_cli(&app)?)
    } else {
        solace_infra::integrations::uninstall(&agent, &home)
    };
    result.map_err(|e| e.to_string())
}

/// Re-points installed hooks at this copy of Solace (it may have moved) and refreshes the skill.
/// Release builds only: a dev build must not aim the user's hooks at `target/`.
pub fn refresh_integrations(app: &tauri::AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let (Ok(home), Ok(exe)) = (home(), hook_cli(app)) else {
        return;
    };
    for agent in solace_infra::integrations::AGENTS {
        if solace_infra::integrations::installed(agent, &home) {
            let _ = solace_infra::integrations::install(agent, &home, &exe);
        }
    }
}
