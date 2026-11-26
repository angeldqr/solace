mod agents;
mod commands;
mod files;
mod notify;
mod orchestration;
mod settings;
mod usage;

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
    agents: HashMap<PaneId, agents::AgentEntry>,
    /// Names given to agents by `solace agent start`; runtime only.
    names: HashMap<PaneId, String>,
    /// States agents announced through their hooks, with when; they outrank screen reading.
    reports: HashMap<PaneId, agents::Report>,
    watchers: files::Watchers,
    file_events: std::sync::mpsc::Sender<solace_core::spaces::SpaceId>,
    file: PathBuf,
    settings: settings::AppSettings,
    settings_file: PathBuf,
    /// "en" or "es", reported by the UI.
    locale: String,
    usage: usage::UsageState,
}

/// Where the `solace` CLI reaches this app; fixed for the life of the process.
pub(crate) fn endpoint() -> &'static str {
    static ENDPOINT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ENDPOINT.get_or_init(|| {
        use std::hash::{BuildHasher, Hasher};
        // RandomState is seeded from the OS: enough to make the name unguessable.
        let nonce = std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish();
        solace_infra::ipc::endpoint(nonce)
    })
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
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Before any pty exists. The fallback is the ConPTY built into Windows.
            #[cfg(windows)]
            if let Err(err) = app
                .path()
                .resource_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| solace_infra::pty::use_bundled_conpty(&dir.join("conpty")))
            {
                eprintln!("bundled ConPTY unavailable, using the Windows one: {err}");
            }
            let data = app.path().app_data_dir()?;
            let file = data.join("session.json");
            let settings_file = data.join("settings.json");
            let settings = persist::load(&settings_file)?.unwrap_or_default();
            // An unreadable session aborts startup rather than being replaced by an empty one.
            let session = persist::load(&file)?.unwrap_or_default();
            app.manage(Mutex::new(AppState {
                session,
                ptys: HashMap::new(),
                agents: HashMap::new(),
                names: HashMap::new(),
                reports: HashMap::new(),
                watchers: Default::default(),
                file_events: files::spawn_relay(app.handle().clone()),
                file,
                settings,
                settings_file,
                locale: "en".into(),
                usage: Default::default(),
            }));
            files::sync_watchers(&app.state::<Shared>().lock().unwrap());
            let handle = app.handle().clone();
            if let Err(err) = solace_infra::ipc::serve(endpoint(), move |request| {
                orchestration::handle(&handle, request)
            }) {
                // The app still works; only `solace` commands are unavailable.
                eprintln!("orchestration API unavailable: {err}");
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || agents::watch(handle));
            let handle = app.handle().clone();
            std::thread::spawn(move || usage::watch(handle));
            let handle = app.handle().clone();
            std::thread::spawn(move || settings::refresh_claude_integration(&handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::session_get,
            commands::agents_get,
            usage::usage_get,
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
            files::files_list,
            files::git_status,
            files::git_diff,
            files::file_open,
            files::file_read,
            files::open_url,
            settings::shells_list,
            settings::settings_get,
            settings::shell_set,
            settings::notifications_set,
            settings::locale_set,
            settings::system_toasts_enabled,
            settings::claude_integration_installed,
            settings::claude_integration_set,
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
