use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use solace_core::spaces::SpaceId;
use solace_infra::files::{self, Entry};
use solace_infra::git::{self, Change};
use solace_infra::watch;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::commands::view;
use crate::{AppState, Shared};

type Result<T> = std::result::Result<T, String>;

/// Bursts of file events (a build, a checkout) collapse into one refresh.
const SETTLE: Duration = Duration::from_millis(250);

/// Opened with their default program on double-click. Only document and source types whose
/// usual handler is a viewer or an editor: many others (.py, .sh, .js, .url, .msix, ...) are
/// associated with something that runs them, so anything not listed is revealed in Explorer.
const OPENABLE: &[&str] = &[
    "bmp", "c", "cc", "cpp", "cs", "css", "csv", "gif", "go", "h", "hpp", "htm", "html", "ico",
    "ini", "java", "jpeg", "jpg", "json", "jsonc", "jsx", "kt", "less", "lock", "log", "md", "mdx",
    "pdf", "png", "rs", "sass", "scss", "sql", "svg", "swift", "toml", "ts", "tsx", "txt", "webp",
    "xml", "yaml", "yml",
];

/// `None`: the space's folder could not be watched.
pub type Watchers = std::sync::Mutex<HashMap<SpaceId, Option<notify::RecommendedWatcher>>>;

fn root_of(state: &State<'_, Shared>, space: SpaceId) -> Result<PathBuf> {
    let state = state.lock().unwrap();
    let space = state
        .session
        .spaces
        .iter()
        .find(|s| s.id == space)
        .ok_or("unknown space")?;
    Ok(space.root.clone())
}

/// `relative` resolved under `root`; anything that could climb out (`..`, absolute paths,
/// drive prefixes) is refused.
fn inside(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(format!("{} is outside the space", relative.display()));
    }
    Ok(root.join(relative))
}

/// Disk and git work runs on the blocking pool so a slow repository never starves the async
/// workers that carry every other command.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn files_list(
    state: State<'_, Shared>,
    space: SpaceId,
    path: String,
) -> Result<Vec<Entry>> {
    let dir = inside(&root_of(&state, space)?, &path)?;
    blocking(move || files::list(&dir).map_err(|e| e.to_string())).await
}

/// `None` when the space is not a git repository.
#[tauri::command]
pub async fn git_status(state: State<'_, Shared>, space: SpaceId) -> Result<Option<Vec<Change>>> {
    let root = root_of(&state, space)?;
    blocking(move || git::status(&root)).await
}

#[tauri::command]
pub async fn git_diff(
    state: State<'_, Shared>,
    space: SpaceId,
    path: String,
    from: Option<String>,
    untracked: bool,
) -> Result<String> {
    // Repository-relative, as git status reported it.
    inside(Path::new(""), &path)?;
    if let Some(from) = &from {
        inside(Path::new(""), from)?;
    }
    let root = root_of(&state, space)?;
    blocking(move || git::diff(&root, &path, from.as_deref(), untracked)).await
}

#[tauri::command]
pub fn file_open(
    app: AppHandle,
    state: State<'_, Shared>,
    space: SpaceId,
    path: String,
) -> Result<()> {
    let target = inside(&root_of(&state, space)?, &path)?;
    let openable = target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OPENABLE.contains(&e.to_ascii_lowercase().as_str()));
    let opener = app.opener();
    let result = if openable {
        opener.open_path(target.to_string_lossy(), None::<&str>)
    } else {
        opener.reveal_item_in_dir(&target)
    };
    result.map_err(|e| e.to_string())
}

/// Keeps one recursive watcher per open space: new spaces start one, closed spaces drop theirs.
/// A folder that can't be watched (gone, offline share) is remembered as such rather than
/// retried on every change, which could stall the app on a network timeout.
pub fn sync_watchers(state: &AppState) {
    let mut watchers = state.watchers.lock().unwrap();
    let open: HashSet<SpaceId> = state.session.spaces.iter().map(|s| s.id).collect();
    watchers.retain(|id, _| open.contains(id));
    for space in &state.session.spaces {
        if watchers.contains_key(&space.id) {
            continue;
        }
        let events = state.file_events.clone();
        let id = space.id;
        let git_dir = git::external_git_dir(&space.root);
        let watcher = watch::watch(&space.root, git_dir.as_deref(), move || {
            let _ = events.send(id);
        });
        watchers.insert(space.id, watcher.ok());
    }
}

/// Coalesces file events and tells the UI which spaces changed. The session is re-sent too,
/// since a checkout or commit can change the branch shown next to each space.
fn relay_changes(app: AppHandle, events: mpsc::Receiver<SpaceId>) {
    while let Ok(first) = events.recv() {
        let mut changed = HashSet::from([first]);
        let deadline = Instant::now() + SETTLE;
        while let Ok(space) =
            events.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            changed.insert(space);
        }
        let _ = app.emit("files-changed", changed.into_iter().collect::<Vec<_>>());
        // Emitted under the lock, like every other session update, so it can't overtake a newer one.
        let state = app.state::<Shared>();
        let state = state.lock().unwrap();
        let _ = app.emit("session", view(&state.session));
    }
}

pub fn spawn_relay(app: AppHandle) -> mpsc::Sender<SpaceId> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || relay_changes(app, rx));
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_leave_the_space() {
        let root = Path::new("C:/work/app");
        assert!(inside(root, "src/main.rs").is_ok());
        assert_eq!(inside(root, "").unwrap(), root);
        assert!(inside(root, "../secret").is_err());
        assert!(inside(root, "src/../../x").is_err());
        // A drive path is absolute on Windows only; elsewhere it names a folder called "C:".
        #[cfg(windows)]
        assert!(inside(root, "C:/Windows").is_err());
        assert!(inside(root, "/etc/passwd").is_err());
    }
}
