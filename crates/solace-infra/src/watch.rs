use std::path::{Component, Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// Calls `on_change` when something that matters under `root` changes; dropping the watcher
/// stops it. Callers debounce: one save can produce several events.
///
/// Ignored: build output and other gitignored paths (a build would fire hundreds of refreshes)
/// and git's internals, except the files that mean "status or branch changed". `git_dir` is
/// watched too when the repository lives outside `root` (worktrees, submodules, subfolders).
pub fn watch(
    root: &Path,
    git_dir: Option<&Path>,
    on_change: impl Fn() + Send + 'static,
) -> notify::Result<RecommendedWatcher> {
    let base: PathBuf = root.to_path_buf();
    let ignored = gitignore(root);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        let relevant = event.paths.iter().any(|path| {
            let Ok(rel) = path.strip_prefix(&base) else {
                // Outside the root: the separately watched git dir, which reports its own files.
                return path
                    .file_name()
                    .is_some_and(|n| n == "HEAD" || n == "index");
            };
            // Worktrees of parallel tasks live here: their churn is not the user's.
            if rel.starts_with(".solace") {
                return false;
            }
            git_state_changed(rel)
                || (!inside_git_dir(rel)
                    && !ignored.matched_path_or_any_parents(rel, false).is_ignore())
        });
        if relevant {
            on_change();
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    if let Some(git_dir) = git_dir {
        watcher.watch(git_dir, RecursiveMode::NonRecursive)?;
    }
    Ok(watcher)
}

/// The root's own .gitignore; nested ones are rare enough to cost only an extra refresh.
fn gitignore(root: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    builder.add(root.join(".gitignore"));
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn inside_git_dir(path: &Path) -> bool {
    path.components()
        .any(|c| c == Component::Normal(".git".as_ref()))
}

/// HEAD (checkout), index (add, commit) and refs (commit, branch) under the root's .git mean
/// the Changes view and the branch label are stale.
fn git_state_changed(path: &Path) -> bool {
    let mut after_git = path
        .components()
        .skip_while(|c| *c != Component::Normal(".git".as_ref()));
    after_git.next().is_some()
        && matches!(
            after_git.next(),
            Some(Component::Normal(n)) if n == "HEAD" || n == "index" || n == "refs" || n == "packed-refs"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_meaningful_git_files_count() {
        assert!(git_state_changed(Path::new(".git/HEAD")));
        assert!(git_state_changed(Path::new(".git/index")));
        assert!(git_state_changed(Path::new(".git/refs/heads/main")));
        assert!(!git_state_changed(Path::new(".git/objects/ab/cdef")));
        assert!(!git_state_changed(Path::new(
            ".git/fsmonitor--daemon/cookies/1"
        )));
        assert!(!git_state_changed(Path::new("src/main.rs")));
        assert!(
            !git_state_changed(Path::new("docs/index")),
            "a file named index is not git's"
        );
        assert!(Path::new(".solace/worktrees/b1-1/a.rs").starts_with(".solace"));
        assert!(!Path::new(".solaceignore").starts_with(".solace"));
        assert!(inside_git_dir(Path::new(".git/objects/ab")));
        assert!(!inside_git_dir(Path::new("src/.gitignore")));
    }
}
