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
