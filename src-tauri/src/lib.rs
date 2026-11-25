mod commands;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use solace_core::session::Session;
use solace_core::terminals::PaneId;
use solace_infra::persist;
use solace_infra::pty::Pty;
use tauri::{Manager, RunEvent};

pub struct AppState {
    session: Session,
    ptys: HashMap<PaneId, Pty>,
    file: PathBuf,
}

type Shared = Mutex<AppState>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // Must be first: a second instance would start duplicate shells and overwrite the session.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let file = app.path().app_data_dir()?.join("session.json");
            // An unreadable session aborts startup rather than being replaced by an empty one.
            let session = persist::load(&file)?.unwrap_or_default();
            app.manage(Mutex::new(AppState {
                session,
                ptys: HashMap::new(),
                file,
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::session_get,
            commands::space_open,
            commands::space_close,
            commands::space_activate,
            commands::tab_new,
            commands::tab_activate,
            commands::tab_close,
            commands::pane_split,
            commands::pane_close,
            commands::pane_focus,
            commands::split_resize,
            commands::pty_attach,
            commands::pty_write,
            commands::pty_resize,
            commands::pty_pause,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Solace");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // Draining first tells the exit handlers this is a shutdown, so the saved
            // session keeps every pane for the next launch.
            let ptys: Vec<Pty> = app
                .state::<Shared>()
                .lock()
                .unwrap()
                .ptys
                .drain()
                .map(|(_, p)| p)
                .collect();
            ptys.into_iter().for_each(Pty::kill);
        }
    });
}
