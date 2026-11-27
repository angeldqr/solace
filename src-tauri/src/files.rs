use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
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
    ignore_whitespace: bool,
) -> Result<String> {
    // Repository-relative, as git status reported it.
    inside(Path::new(""), &path)?;
    if let Some(from) = &from {
        inside(Path::new(""), from)?;
    }
    let root = root_of(&state, space)?;
    blocking(move || git::diff(&root, &path, from.as_deref(), untracked, ignore_whitespace)).await
}

/// The image as of the last commit, as a data URL; `None` when it didn't exist or is too big.
#[tauri::command]
pub async fn git_show_image(
    state: State<'_, Shared>,
    space: SpaceId,
    path: String,
) -> Result<Option<String>> {
    inside(Path::new(""), &path)?;
    let mime = image_mime(&path).ok_or("not an image")?;
    let root = root_of(&state, space)?;
    blocking(move || {
        Ok(git::show_image(&root, &path, MAX_IMAGE as usize)?
            .map(|bytes| format!("data:{mime};base64,{}", STANDARD.encode(bytes))))
    })
    .await
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

const MAX_TEXT: u64 = 2 * 1024 * 1024;
const MAX_IMAGE: u64 = 10 * 1024 * 1024;

/// What the in-app viewer can show for a file.
#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileContent {
    Text {
        text: String,
        size: u64,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        data_url: String,
        size: u64,
    },
    Binary {
        size: u64,
    },
    TooLarge {
        size: u64,
    },
}

fn image_mime(name: &str) -> Option<&'static str> {
    Some(
        match name.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "bmp" => "image/bmp",
            "ico" => "image/x-icon",
            "svg" => "image/svg+xml",
            _ => return None,
        },
    )
}

/// Pure part of `file_read`: size limits are checked by the caller before reading.
fn classify(name: &str, bytes: Vec<u8>) -> FileContent {
    let size = bytes.len() as u64;
    if let Some(mime) = image_mime(name) {
        let data_url = format!("data:{mime};base64,{}", STANDARD.encode(&bytes));
        return FileContent::Image { data_url, size };
    }
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return FileContent::Binary { size };
    }
    match String::from_utf8(bytes) {
        Ok(text) => FileContent::Text {
            text: text
                .strip_prefix('\u{feff}')
                .map(str::to_owned)
                .unwrap_or(text),
            size,
        },
        Err(_) => FileContent::Binary { size },
    }
}

/// `target` with links resolved, refused when it lands outside `root` (a symlink or junction in the space).
fn contained(root: &Path, target: &Path) -> Result<PathBuf> {
    let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let target = std::fs::canonicalize(target).map_err(|e| e.to_string())?;
    if !target.starts_with(&root) {
        return Err(format!("{} is outside the space", target.display()));
    }
    Ok(target)
}

/// Only web and mail links leave the app; anything else (file:, javascript:, custom schemes) is refused.
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if !["http://", "https://", "mailto:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        return Err("unsupported link".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn file_read(
    state: State<'_, Shared>,
    space: SpaceId,
    path: String,
) -> Result<FileContent> {
    let root = root_of(&state, space)?;
    let target = inside(&root, &path)?;
    blocking(move || {
        let target = contained(&root, &target)?;
        if std::fs::metadata(&target)
            .map_err(|e| e.to_string())?
            .is_dir()
        {
            return Err("is a folder".into());
        }
        let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let limit = if image_mime(name).is_some() {
            MAX_IMAGE
        } else {
            MAX_TEXT
        };
        // The size on disk can lag or lie (growing logs, special files): never read past the limit.
        let mut bytes = Vec::new();
        File::open(&target)
            .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > limit {
            let size = std::fs::metadata(&target)
                .map(|m| m.len())
                .unwrap_or(limit + 1);
            return Ok(FileContent::TooLarge { size });
        }
        Ok(classify(name, bytes))
    })
    .await
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

    #[test]
    fn links_cannot_leave_the_space() {
        let base = std::env::temp_dir().join(format!("solace-contained-{}", std::process::id()));
        let (root, outside) = (base.join("root"), base.join("outside"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("a.txt"), "a").unwrap();
        std::fs::write(outside.join("b.txt"), "b").unwrap();
        assert!(contained(&root, &root.join("a.txt")).is_ok());
        assert!(contained(&root, &outside.join("b.txt")).is_err());
        // Creating symlinks needs privileges on Windows, so the link case runs elsewhere only.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("b.txt"), root.join("link")).unwrap();
            assert!(contained(&root, &root.join("link")).is_err());
        }
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn classifies_file_contents() {
        let text = |s: &str| FileContent::Text {
            text: s.into(),
            size: s.len() as u64,
        };
        assert_eq!(
            classify("a.rs", b"fn main() {}".to_vec()),
            text("fn main() {}")
        );
        let bom = classify("a.md", b"\xEF\xBB\xBFhi".to_vec());
        assert_eq!(
            bom,
            FileContent::Text {
                text: "hi".into(),
                size: 5
            }
        );
        assert_eq!(
            classify("a.bin", vec![1, 0, 2]),
            FileContent::Binary { size: 3 }
        );
        assert_eq!(
            classify("a.txt", vec![0xff, 0xfe, 0x41]),
            FileContent::Binary { size: 3 }
        );
        let FileContent::Image { data_url, .. } = classify("X.PNG", vec![1, 2, 3]) else {
            panic!("png is an image")
        };
        assert_eq!(data_url, "data:image/png;base64,AQID");
        let FileContent::Image { data_url, .. } = classify("a.svg", b"<svg/>".to_vec()) else {
            panic!("svg is an image")
        };
        assert!(data_url.starts_with("data:image/svg+xml;base64,"));
    }
}
