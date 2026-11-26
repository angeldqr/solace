use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

use serde::Serialize;

/// Branch checked out in the repository containing `dir`, or a short commit id when detached.
/// Reads `.git/HEAD` directly: no process spawn, safe to call on every refresh.
pub fn current_branch(dir: &Path) -> Option<String> {
    dir.ancestors().find_map(|d| {
        let dot_git = d.join(".git");
        let git_dir = if dot_git.is_file() {
            // Worktrees and submodules: `.git` is a file pointing at the real git dir.
            let pointer = fs::read_to_string(&dot_git).ok()?;
            d.join(pointer.strip_prefix("gitdir:")?.trim())
        } else {
            dot_git
        };
        let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
        let head = head.trim();
        match head.strip_prefix("ref: refs/heads/") {
            Some(branch) => Some(branch.to_string()),
            None => head.get(..7).map(str::to_string),
        }
    })
}

/// A changed file, relative to the repository root.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    /// M modified, A added, D deleted, R renamed, U untracked, C copied, T type change,
    /// ! merge conflict.
    pub status: char,
    /// The path before a rename.
    pub from: Option<String>,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

fn git(dir: &Path, args: &[&str]) -> io::Result<Output> {
    let mut command = Command::new("git");
    // Read-only: without this, `git status` rewrites .git/index, which the file watcher would
    // report, which would trigger another status, forever.
    // The repository's own config is not trusted: a folder from anywhere could set
    // core.fsmonitor (also a refresh loop: its cookie files trip our watcher), an external diff
    // or a textconv driver, all of which run programs.
    command
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A GUI app starting a console program would flash a window without this.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output()
}

fn git_text(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = git(dir, args).map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => "git is not installed".to_string(),
        _ => err.to_string(),
    })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Uncommitted changes under `dir`, staged or not, with line counts. `Ok(None)` outside a repo.
pub fn status(dir: &Path) -> Result<Option<Vec<Change>>, String> {
    if current_branch(dir).is_none() {
        return Ok(None);
    }
    let porcelain = git_text(
        dir,
        &[
            "-c",
            "core.quotepath=off",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    )?;
    let mut changes = parse_status(&porcelain);
    // Fails in a repository without commits yet; the counts are then just left out.
    if let Ok(numstat) = git_text(
        dir,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "HEAD",
            "--numstat",
            "-z",
            "--",
            ".",
        ],
    ) {
        let counts: std::collections::HashMap<String, Counts> =
            parse_numstat(&numstat).into_iter().collect();
        for change in &mut changes {
            if let Some(&(add, del)) = counts.get(&change.path) {
                change.additions = add;
                change.deletions = del;
            }
        }
    }
    let top = git_text(dir, &["rev-parse", "--show-toplevel"])?;
    for change in changes.iter_mut().filter(|c| c.status == 'U') {
        // Untracked files have no diff stat: count their lines (small text files only).
        let path = Path::new(top.trim()).join(&change.path);
        // Not following links: a link to a device or network share must not be read.
        if let Ok(meta) = fs::symlink_metadata(&path)
            && meta.is_file()
            && meta.len() <= MAX_TEXT
            && let Ok(text) = fs::read_to_string(&path)
        {
            change.additions = Some(text.lines().count() as u32);
            change.deletions = Some(0);
        }
    }
    Ok(Some(changes))
}

/// The repository's git directory, when it lives outside `dir` (`dir` is a subfolder of the
/// repository, a worktree or a submodule): its HEAD and index must be watched separately.
pub fn external_git_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let git_dir = git_text(dir, &["rev-parse", "--absolute-git-dir"]).ok()?;
    let git_dir = std::path::PathBuf::from(git_dir.trim());
    let dir = dir.canonicalize().ok()?;
    (!git_dir.canonicalize().ok()?.starts_with(&dir)).then_some(git_dir)
}

/// Files and diffs bigger than this are not read or shown in full.
const MAX_TEXT: u64 = 1024 * 1024;

/// Unified diff of one file against the last commit; untracked files diff against nothing.
/// A renamed file (`from` its old path) diffs against its old contents, not against nothing.
/// Capped at MAX_TEXT so a huge file can't freeze the UI.
pub fn diff(dir: &Path, path: &str, from: Option<&str>, untracked: bool) -> Result<String, String> {
    // literal: names like app/[id]/page.tsx must not act as glob patterns.
    let literal = |p: &str| format!(":(top,literal){p}");
    let mut paths = vec![literal(path)];
    paths.extend(from.map(literal));
    let mut text = if untracked {
        let top = git_text(dir, &["rev-parse", "--show-toplevel"])?;
        let full = Path::new(top.trim()).join(path);
        if fs::symlink_metadata(&full).is_ok_and(|m| m.len() > MAX_TEXT) {
            return Ok(String::new());
        }
        let output = git(
            Path::new(top.trim()),
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-index",
                "--",
                "/dev/null",
                path,
            ],
        )
        .map_err(|e| e.to_string())?;
        // --no-index exits with 1 when the files differ, which is the expected case here.
        match output.status.code() {
            Some(0 | 1) => String::from_utf8_lossy(&output.stdout).into_owned(),
            _ => return Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        }
    } else {
        git_text(
            dir,
            &[
                "-c",
                "core.quotepath=off",
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "-M",
                "HEAD",
                "--",
            ]
            .into_iter()
            .chain(paths.iter().map(String::as_str))
            .collect::<Vec<_>>(),
        )?
    };
    if text.len() as u64 > MAX_TEXT {
        let mut cut = MAX_TEXT as usize;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    Ok(text)
}

fn parse_status(porcelain: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut fields = porcelain.split('\0').filter(|f| !f.is_empty());
    while let Some(entry) = fields.next() {
        let (Some(code), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        let mut chars = code.chars();
        let (index, worktree) = (chars.next().unwrap_or(' '), chars.next().unwrap_or(' '));
        // Renames and copies are followed by the original path.
        let from = (matches!(index, 'R' | 'C') || matches!(worktree, 'R' | 'C'))
            .then(|| fields.next().map(str::to_string))
            .flatten();
        let status = match (index, worktree) {
            ('?', _) => 'U',
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => '!',
            (' ', w) => w,
            (i, _) => i,
        };
        changes.push(Change {
            path: path.to_string(),
            status,
            // A copy's source still exists with its own changes; only renames diff against it.
            from: from.filter(|_| status == 'R'),
            additions: None,
            deletions: None,
        });
    }
    changes
}

type Counts = (Option<u32>, Option<u32>);

fn parse_numstat(numstat: &str) -> Vec<(String, Counts)> {
    let mut counts = Vec::new();
    let mut fields = numstat.split('\0');
    while let Some(entry) = fields.next() {
        let mut parts = entry.splitn(3, '\t');
        let (Some(add), Some(del), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        // A rename leaves the path empty and gives old and new paths as the next two fields.
        let path = if path.is_empty() {
            fields.next();
            fields.next().unwrap_or_default().to_string()
        } else {
            path.to_string()
        };
        // Binary files report "-" for both counts.
        counts.push((path, (add.parse().ok(), del.parse().ok())));
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_status_and_numstat() {
        let status = parse_status(
            " M src/a.rs\0A  new.rs\0R  renamed.rs\0old.rs\0?? notes.md\0 D gone.rs\0UU both.rs\0",
        );
        let summary: Vec<(&str, char)> =
            status.iter().map(|c| (c.path.as_str(), c.status)).collect();
        assert_eq!(
            summary,
            vec![
                ("src/a.rs", 'M'),
                ("new.rs", 'A'),
                ("renamed.rs", 'R'),
                ("notes.md", 'U'),
                ("gone.rs", 'D'),
                ("both.rs", '!')
            ]
        );
        let counts = parse_numstat("3\t1\tsrc/a.rs\0-\t-\tlogo.png\0");
        assert_eq!(counts[0], ("src/a.rs".to_string(), (Some(3), Some(1))));
        assert_eq!(counts[1], ("logo.png".to_string(), (None, None)));
        assert_eq!(status[2].from.as_deref(), Some("old.rs"));
        let renamed = parse_numstat("0\t0\t\0old.rs\0renamed.rs\0");
        assert_eq!(renamed[0].0, "renamed.rs");
    }

    #[test]
    fn status_and_diff_on_a_real_repository() {
        if git(&std::env::temp_dir(), &["--version"]).is_err() {
            eprintln!("skipped: git is not installed");
            return;
        }
        let root = std::env::temp_dir().join(format!("solace-git-repo-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let run =
            |args: &[&str]| assert!(git(&root, args).unwrap().status.success(), "git {args:?}");
        run(&["init", "-q"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ]);
        fs::write(root.join("tracked.txt"), "one\n").unwrap();
        run(&["add", "tracked.txt"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "add",
        ]);
        fs::write(root.join("tracked.txt"), "one\ntwo\nthree\n").unwrap();
        fs::write(root.join("new.txt"), "a\nb\n").unwrap();

        let changes = status(&root).unwrap().unwrap();
        let tracked = changes.iter().find(|c| c.path == "tracked.txt").unwrap();
        assert_eq!(
            (tracked.status, tracked.additions, tracked.deletions),
            ('M', Some(2), Some(0))
        );
        let new = changes.iter().find(|c| c.path == "new.txt").unwrap();
        assert_eq!((new.status, new.additions), ('U', Some(2)));

        assert!(
            diff(&root, "tracked.txt", None, false)
                .unwrap()
                .contains("+three")
        );
        assert!(diff(&root, "new.txt", None, true).unwrap().contains("+b"));
        run(&["mv", "tracked.txt", "moved.txt"]);
        // Back to the committed text: a pure rename, which git must report as one.
        fs::write(root.join("moved.txt"), "one\n").unwrap();
        let moved = status(&root).unwrap().unwrap();
        let moved = moved.iter().find(|c| c.path == "moved.txt").unwrap();
        assert_eq!(
            (moved.status, moved.from.as_deref()),
            ('R', Some("tracked.txt"))
        );
        let renamed = diff(&root, "moved.txt", Some("tracked.txt"), false).unwrap();
        assert!(renamed.contains("rename from tracked.txt"), "{renamed}");
        assert!(
            !renamed.contains("+one"),
            "a rename is not shown as a whole new file"
        );
        assert_eq!(
            status(&std::env::temp_dir().join("solace-not-a-repo-xyz"))
                .ok()
                .flatten(),
            None
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_branch_from_head_and_from_subfolders() {
        let root = std::env::temp_dir().join(format!("solace-git-test-{}", std::process::id()));
        let sub = root.join("src/deep");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(&sub).unwrap();

        fs::write(root.join(".git/HEAD"), "ref: refs/heads/feat/pty\n").unwrap();
        assert_eq!(current_branch(&sub).as_deref(), Some("feat/pty"));

        fs::write(root.join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(current_branch(&root).as_deref(), Some("0123456"));

        fs::remove_dir_all(&root).unwrap();
    }
}
