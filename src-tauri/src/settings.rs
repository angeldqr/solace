use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use solace_infra::persist;
use solace_infra::pty::{Shell, available_shells};
use tauri::State;

use crate::Shared;

/// Settings the backend needs; look-and-feel settings live in the UI.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Shell for new terminals; the platform default when unset.
    #[serde(default)]
    pub shell: Option<PathBuf>,
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
