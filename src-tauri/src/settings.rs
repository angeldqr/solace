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

fn claude_paths() -> Result<solace_infra::integrations::ClaudePaths, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("no home directory")?;
    Ok(solace_infra::integrations::ClaudePaths::in_home(
        std::path::Path::new(&home),
    ))
}

#[tauri::command(async)]
pub fn claude_integration_installed() -> Result<bool, String> {
    Ok(solace_infra::integrations::claude_installed(
        &claude_paths()?
    ))
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

#[tauri::command(async)]
pub fn claude_integration_set(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let paths = claude_paths()?;
    let result = if enabled {
        solace_infra::integrations::install_claude(&paths, &bundled_cli(&app)?)
    } else {
        solace_infra::integrations::uninstall_claude(&paths)
    };
    result.map_err(|e| e.to_string())
}

/// Re-points installed hooks at this copy of Solace (it may have moved) and refreshes the skill.
/// Release builds only: a dev build must not aim the user's hooks at `target/`.
pub fn refresh_claude_integration(app: &tauri::AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let (Ok(paths), Ok(exe)) = (claude_paths(), bundled_cli(app)) else {
        return;
    };
    if solace_infra::integrations::claude_installed(&paths) {
        let _ = solace_infra::integrations::install_claude(&paths, &exe);
    }
}
