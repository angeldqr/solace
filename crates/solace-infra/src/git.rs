use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use serde::Serialize;

/// Branch checked out in the repository containing `dir`, or a short commit id when detached.
/// Reads `.git/HEAD` directly: no process spawn, safe to call on every refresh.
pub fn current_branch(dir: &Path) -> Option<String> {
    dir.ancestors().find_map(|d| {
        let dot_git = d.join(".git");
        let git_dir = if dot_git.is_file() {
            gitdir_pointer(d)?
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

/// Where the `.git` file in `d` points: worktrees and submodules keep the real git dir elsewhere.
/// A folder can't send us to a network share, or to a place that is no ancestor of itself (a
/// worktree's git dir is inside its repository, which is one).
fn gitdir_pointer(d: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(d.join(".git")).ok()?;
    let target = text.strip_prefix("gitdir:")?.trim();
    if target.starts_with("\\\\") || target.starts_with("//") {
        return None;
    }
    let path = Path::new(target);
    // Also "\\x" and "C:x", which `join` treats as moving off the folder.
    if matches!(
        path.components().next(),
        Some(std::path::Component::RootDir | std::path::Component::Prefix(_))
    ) {
        let inside = format!("{}/", normal(target));
        let under_ancestor = d
            .ancestors()
            .filter(|a| a.parent().is_some())
            .any(|a| inside.starts_with(&format!("{}/", normal(&a.to_string_lossy()))));
        if !under_ancestor {
            return None;
        }
    }
    Some(d.join(path))
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

fn command(dir: &Path) -> Command {
    let mut command = windowless("git");
    command.current_dir(dir);
    command
}

fn windowless(program: &str) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A GUI app starting a console program would flash a window without this.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn git(dir: &Path, args: &[&str]) -> io::Result<Output> {
    let mut command = command(dir);
    // Read-only: without this, `git status` rewrites .git/index, which the file watcher would
    // report, which would trigger another status, forever.
    // The repository's own config is not trusted: a folder from anywhere could set
    // core.fsmonitor (also a refresh loop: its cookie files trip our watcher), an external diff,
    // a textconv driver or a filter driver, all of which run programs. Submodules, whose config
    // we don't read, are only compared by commit (--ignore-submodules=dirty at the call sites).
    command
        .args(["--no-optional-locks", "-c", "core.fsmonitor=false"])
        .args(args);
    // `-c` can't name drivers it doesn't know, so the filters the config defines are blanked one
    // by one, through GIT_CONFIG_* so that a driver name holding '=' can't break the parsing.
    let mut count = 0;
    for name in filter_drivers(dir) {
        for (key, value) in [
            ("clean", ""),
            ("smudge", ""),
            ("process", ""),
            ("required", "false"),
        ] {
            command
                .env(
                    format!("GIT_CONFIG_KEY_{count}"),
                    format!("filter.{name}.{key}"),
                )
                .env(format!("GIT_CONFIG_VALUE_{count}"), value);
            count += 1;
        }
    }
    command.env("GIT_CONFIG_COUNT", count.to_string());
    command.output()
}

/// Names of the filter drivers the config seen from `dir` defines. Reading config runs nothing.
fn filter_drivers(dir: &Path) -> Vec<String> {
    let Ok(output) = command(dir)
        .args(["config", "-z", "--name-only", "--get-regexp", r"^filter\."])
        .output()
    else {
        return Vec::new();
    };
    let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|key| Some(key.strip_prefix("filter.")?.rsplit_once('.')?.0.to_string()))
        .collect();
    names.sort();
    names.dedup();
    names
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

/// The repository's top folder; `Ok(None)` when `dir` is not inside one. A git that can't run
/// is an error, not "no repository". With `wsl` git runs in WSL, and the folder is translated
/// back to a Windows path (a repository inside the distro's own file system is refused).
pub fn toplevel(dir: &Path, wsl: bool) -> Result<Option<PathBuf>, String> {
    // The files say whether there is a repository; git's own message is localized.
    if current_branch(dir).is_none() {
        return Ok(None);
    }
    let top = text(dir, wsl, &["rev-parse", "--show-toplevel"])?;
    let top = top.trim();
    if wsl {
        from_wsl_path(top).map(Some)
    } else {
        Ok(Some(PathBuf::from(top)))
    }
}

/// `/mnt/c/Users/x` is `C:/Users/x`.
fn from_wsl_path(path: &str) -> Result<PathBuf, String> {
    let rest = path.strip_prefix("/mnt/").ok_or_else(|| {
        format!("{path} is inside WSL's own file system; open the repository from a Windows drive")
    })?;
    let (drive, tail) = rest.split_once('/').unwrap_or((rest, ""));
    if drive.len() != 1 {
        return Err(format!("{path} is not on a Windows drive"));
    }
    Ok(PathBuf::from(format!(
        "{}:/{tail}",
        drive.to_ascii_uppercase()
    )))
}

/// Short id of the current commit; `Err` in a repository without commits.
pub fn head(top: &Path, wsl: bool) -> Result<String, String> {
    text(top, wsl, &["rev-parse", "--short", "HEAD"]).map(|id| id.trim().to_string())
}

/// How many commits `branch` has that `base` doesn't.
pub fn commits_since(top: &Path, base: &str, branch: &str, wsl: bool) -> Result<u32, String> {
    let range = format!("{base}..{branch}");
    let count = text(top, wsl, &["rev-list", "--count", &range])?;
    count
        .trim()
        .parse()
        .map_err(|_| format!("unexpected count {count:?}"))
}

/// Whether `dir` has uncommitted changes, untracked files included.
pub fn is_dirty(dir: &Path, wsl: bool) -> Result<bool, String> {
    if wsl {
        let out = text(
            dir,
            true,
            &["status", "--porcelain", "--untracked-files=all"],
        )?;
        return Ok(!out.trim().is_empty());
    }
    Ok(status(dir)?.is_some_and(|changes| !changes.is_empty()))
}

/// Output of `git <args>` in `dir`, through WSL when the agents run there.
fn text(dir: &Path, wsl: bool, args: &[&str]) -> Result<String, String> {
    if !wsl {
        return git_text(dir, args);
    }
    let output = worktree_command(dir, true, args)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn run(mut command: Command) -> Result<(), String> {
    let output = command.output().map_err(|err| match err.kind() {
        io::ErrorKind::NotFound => "git is not installed".to_string(),
        _ => err.to_string(),
    })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// `git <args>` in `top`, through WSL when the agents run there.
fn worktree_command(top: &Path, wsl: bool, args: &[&str]) -> Command {
    if wsl {
        let mut command = windowless("wsl.exe");
        command
            .arg("--cd")
            .arg(top)
            .args(["--exec", "git"])
            .args(args);
        command
    } else {
        let mut command = command(top);
        command.args(args);
        command
    }
}

/// The full id of the commit `base` names (a branch, tag or id); errors when there is none.
pub fn resolve_commit(top: &Path, base: &str, wsl: bool) -> Result<String, String> {
    let spec = format!("{base}^{{commit}}");
    text(top, wsl, &["rev-parse", "--verify", "--quiet", &spec])
        .map(|id| id.trim().to_string())
        .map_err(|_| format!("unknown base {base}"))
}

const LINKED: &str = "refusing to create task worktrees: .solace is a link";

/// A symlink, or on Windows a junction or other reparse point.
fn is_link(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true; // FILE_ATTRIBUTE_REPARSE_POINT
        }
    }
    meta.file_type().is_symlink()
}

/// `top/.solace` and `top/.solace/worktrees` must be real folders when they exist.
fn unlinked(top: &Path) -> Result<(), String> {
    let solace = top.join(".solace");
    if is_link(&solace) || is_link(&solace.join("worktrees")) {
        return Err(LINKED.into());
    }
    Ok(())
}

/// Adds a worktree at `top/rel_dir` on a new branch started from `base` (a validated ref). `.solace/` gets its own
/// `.gitignore` so worktrees never show up as changes; the user's files are not touched.
/// Plain `git`, not `git()`: that one blanks the repository's filters, and a worktree checked out
/// without them would hold LFS pointers instead of files. The agents run this repository's tools
/// in there anyway, so trusting its config here adds nothing.
pub fn worktree_add(
    top: &Path,
    rel_dir: &str,
    branch: &str,
    base: &str,
    wsl: bool,
) -> Result<(), String> {
    let solace = top.join(".solace");
    let worktrees = solace.join("worktrees");
    // A committed link could send the worktrees (and `.solace/.gitignore`) out of the repository.
    unlinked(top)?;
    fs::create_dir_all(&worktrees).map_err(|e| e.to_string())?;
    unlinked(top)?;
    let (real, real_top) = (fs::canonicalize(&worktrees), fs::canonicalize(top));
    if !matches!((&real, &real_top), (Ok(r), Ok(t)) if r.starts_with(t)) {
        return Err(LINKED.into());
    }
    let ignore = solace.join(".gitignore");
    if fs::symlink_metadata(&ignore).is_err() {
        fs::write(&ignore, "*\n").map_err(|e| e.to_string())?;
    }
    run(worktree_command(
        top,
        wsl,
        &["worktree", "add", "-b", branch, rel_dir, base],
    ))
}

/// Removes a worktree. Without `force` git refuses one with uncommitted changes.
pub fn worktree_remove(top: &Path, rel_dir: &str, force: bool, wsl: bool) -> Result<(), String> {
    unlinked(top)?;
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(rel_dir);
    run(worktree_command(top, wsl, &args))
}

/// Delays between attempts to remove a worktree whose files are still held by exiting processes.
pub const REMOVE_BACKOFF: [Duration; 4] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

fn normal(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = path.trim_end_matches('/');
    if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.to_string()
    }
}

/// Whether git still lists `dir` as one of its worktrees.
pub fn worktree_registered(top: &Path, dir: &Path, wsl: bool) -> Result<bool, String> {
    let list = text(top, wsl, &["worktree", "list", "--porcelain"])?;
    let want = normal(&dir.to_string_lossy());
    Ok(list
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .filter_map(|path| {
            if wsl {
                from_wsl_path(path)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            } else {
                Some(path.to_string())
            }
        })
        .any(|path| normal(&path) == want))
}

/// Forgets worktrees whose folders are gone.
pub fn worktree_prune(top: &Path, wsl: bool) -> Result<(), String> {
    run(worktree_command(top, wsl, &["worktree", "prune"]))
}

/// Deletes a worktree's folder that git let go of but could not delete (Windows keeps files
/// locked while processes exit). Only folders inside `top/.solace/worktrees` are ever touched.
pub fn remove_leftover(top: &Path, dir: &Path, delays: &[Duration]) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    unlinked(top)?;
    let root = fs::canonicalize(top.join(".solace/worktrees")).map_err(|e| e.to_string())?;
    let target = fs::canonicalize(dir).map_err(|e| e.to_string())?;
    if target == root || !target.starts_with(&root) {
        return Err(format!(
            "{} is not inside {}",
            target.display(),
            root.display()
        ));
    }
    let mut last = String::new();
    for delay in std::iter::once(&Duration::ZERO).chain(delays) {
        std::thread::sleep(*delay);
        match fs::remove_dir_all(&target) {
            Ok(()) => return Ok(()),
            Err(_) if !target.exists() => return Ok(()),
            Err(err) => last = err.to_string(),
        }
    }
    Err(format!("could not delete {}: {last}", target.display()))
}

/// `worktree_remove` for Windows, where a worktree whose processes are still exiting cannot be
/// deleted yet: it retries with backoff, and when git has let go of the worktree but left its
/// folder, deletes the folder itself (see `remove_leftover`). Uncommitted changes without
/// `force` are refused at once, with no retries.
pub fn worktree_remove_retrying(
    top: &Path,
    rel_dir: &str,
    force: bool,
    wsl: bool,
) -> Result<(), String> {
    let dir = top.join(rel_dir);
    let mut result = worktree_remove(top, rel_dir, force, wsl);
    for delay in REMOVE_BACKOFF {
        let Err(err) = &result else { break };
        let lower = err.to_lowercase();
        let locked = [
            "failed to delete",
            "permission denied",
            "another process",
            "not empty",
        ]
        .iter()
        .any(|m| lower.contains(m));
        if !locked || matches!(worktree_registered(top, &dir, wsl), Ok(false)) {
            break;
        }
        std::thread::sleep(delay);
        result = worktree_remove(top, rel_dir, force, wsl);
    }
    let Err(err) = result else { return Ok(()) };
    if matches!(worktree_registered(top, &dir, wsl), Ok(false)) {
        remove_leftover(top, &dir, &REMOVE_BACKOFF)?;
        let _ = worktree_prune(top, wsl);
        return Ok(());
    }
    Err(err)
}

/// Deletes a branch nobody has merged: only for undoing a worktree that was just made.
pub fn branch_delete(top: &Path, branch: &str, wsl: bool) -> Result<(), String> {
    run(worktree_command(top, wsl, &["branch", "-D", branch]))
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
            "--ignore-submodules=dirty",
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
            "--ignore-submodules=dirty",
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
    let nearest = dir.ancestors().find(|d| d.join(".git").exists())?;
    if nearest.join(".git").is_file() && gitdir_pointer(nearest).is_none() {
        return None; // git would follow it
    }
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
pub fn diff(
    dir: &Path,
    path: &str,
    from: Option<&str>,
    untracked: bool,
    ignore_whitespace: bool,
) -> Result<String, String> {
    // `-w` goes before the `--` separator in both branches.
    let flags: &[&str] = if ignore_whitespace { &["-w"] } else { &[] };
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
            &["diff", "--no-ext-diff", "--no-textconv", "--no-index"]
                .into_iter()
                .chain(flags.iter().copied())
                .chain(["--", "/dev/null", path])
                .collect::<Vec<_>>(),
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
                "--ignore-submodules=dirty",
                "-M",
            ]
            .into_iter()
            .chain(flags.iter().copied())
            .chain(["HEAD", "--"])
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

/// Bytes of `path` at HEAD, for previewing images; `None` when it is missing there or over `max`.
pub fn show_image(dir: &Path, path: &str, max: usize) -> Result<Option<Vec<u8>>, String> {
    let spec = format!("HEAD:{}", path.replace('\\', "/"));
    // Ask for the size first so a huge blob is never read.
    let size = git(dir, &["cat-file", "-s", &spec]).map_err(|e| e.to_string())?;
    let size = String::from_utf8_lossy(&size.stdout)
        .trim()
        .parse::<usize>();
    if size.is_ok_and(|n| n > max) {
        return Ok(None);
    }
    let output = git(dir, &["show", "--no-textconv", &spec]).map_err(|e| e.to_string())?;
    Ok((output.status.success() && output.stdout.len() <= max).then_some(output.stdout))
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
            diff(&root, "tracked.txt", None, false, false)
                .unwrap()
                .contains("+three")
        );
        assert!(
            diff(&root, "new.txt", None, true, false)
                .unwrap()
                .contains("+b")
        );
        // -w drops whitespace-only changes: "one " is still "one".
        fs::write(root.join("tracked.txt"), "one \n").unwrap();
        assert!(
            diff(&root, "tracked.txt", None, false, false)
                .unwrap()
                .contains("-one")
        );
        assert!(
            !diff(&root, "tracked.txt", None, false, true)
                .unwrap()
                .contains("-one")
        );
        run(&["mv", "tracked.txt", "moved.txt"]);
        // Back to the committed text: a pure rename, which git must report as one.
        fs::write(root.join("moved.txt"), "one\n").unwrap();
        let moved = status(&root).unwrap().unwrap();
        let moved = moved.iter().find(|c| c.path == "moved.txt").unwrap();
        assert_eq!(
            (moved.status, moved.from.as_deref()),
            ('R', Some("tracked.txt"))
        );
        let renamed = diff(&root, "moved.txt", Some("tracked.txt"), false, false).unwrap();
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
    fn show_image_reads_head_bytes() {
        if git(&std::env::temp_dir(), &["--version"]).is_err() {
            eprintln!("skipped: git is not installed");
            return;
        }
        let root = std::env::temp_dir().join(format!("solace-git-show-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let run =
            |args: &[&str]| assert!(git(&root, args).unwrap().status.success(), "git {args:?}");
        run(&["init", "-q"]);
        fs::write(root.join("a.png"), [1u8, 2, 3]).unwrap();
        run(&["add", "a.png"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "x",
        ]);
        assert_eq!(show_image(&root, "a.png", 10).unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(show_image(&root, "a.png", 2).unwrap(), None);
        assert_eq!(show_image(&root, "none.png", 10).unwrap(), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn worktrees_are_added_hidden_and_removed_only_when_clean() {
        if git(&std::env::temp_dir(), &["--version"]).is_err() {
            eprintln!("skipped: git is not installed");
            return;
        }
        let root = std::env::temp_dir().join(format!("solace-git-wt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let run =
            |args: &[&str]| assert!(git(&root, args).unwrap().status.success(), "git {args:?}");
        run(&["init", "-q"]);
        assert!(head(&root, false).is_err(), "no commits yet");
        fs::write(root.join("f.txt"), "a\n").unwrap();
        run(&["add", "f.txt"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "init",
        ]);
        assert!(!head(&root, false).unwrap().is_empty());
        let top = toplevel(&root, false).unwrap().unwrap();
        assert_eq!(commits_since(&top, "HEAD", "HEAD", false).unwrap(), 0);
        assert!(!is_dirty(&root, false).unwrap());

        let rel = ".solace/worktrees/b1-1";
        assert!(resolve_commit(&top, "nope", false).is_err());
        assert!(resolve_commit(&top, "HEAD", false).is_ok());
        worktree_add(&top, rel, "solace/b1-1-x", "HEAD", false).unwrap();
        assert!(top.join(rel).join("f.txt").is_file());
        assert!(top.join(".solace/.gitignore").is_file());
        assert_eq!(
            current_branch(&top.join(rel)).as_deref(),
            Some("solace/b1-1-x")
        );
        let changes = status(&root).unwrap().unwrap();
        assert!(
            changes.is_empty(),
            "the main folder shows no changes: {changes:?}"
        );

        fs::write(top.join(rel).join("f.txt"), "changed\n").unwrap();
        assert!(is_dirty(&top.join(rel), false).unwrap());
        assert!(
            worktree_remove(&top, rel, false, false).is_err(),
            "dirty: refused"
        );
        assert!(top.join(rel).is_dir());
        worktree_remove(&top, rel, true, false).unwrap();
        assert!(!top.join(rel).exists());
        branch_delete(&top, "solace/b1-1-x", false).unwrap();

        // The retrying removal: refuses dirty work, removes clean worktrees, and cleans up a
        // folder git no longer knows but could not delete.
        let rel2 = ".solace/worktrees/b1-2";
        worktree_add(&top, rel2, "solace/b1-2-y", "HEAD", false).unwrap();
        assert!(worktree_registered(&top, &top.join(rel2), false).unwrap());
        fs::write(top.join(rel2).join("f.txt"), "dirty\n").unwrap();
        assert!(worktree_remove_retrying(&top, rel2, false, false).is_err());
        worktree_remove_retrying(&top, rel2, true, false).unwrap();
        assert!(!top.join(rel2).exists());
        assert!(!worktree_registered(&top, &top.join(rel2), false).unwrap());
        let left = top.join(".solace/worktrees/b1-3");
        fs::create_dir_all(left.join("sub")).unwrap();
        fs::write(left.join("sub/x"), "x").unwrap();
        remove_leftover(&top, &left, &[]).unwrap();
        assert!(!left.exists());
        assert!(
            remove_leftover(&top, &top, &[]).is_err(),
            "outside .solace/worktrees: refused"
        );
        assert!(remove_leftover(&top, &top.join(".solace"), &[]).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_linked_solace_folder_is_refused_and_foreign_gitdirs_ignored() {
        let root = std::env::temp_dir().join(format!("solace-git-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (top, outside) = (root.join("top"), root.join("outside"));
        fs::create_dir_all(&top).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = top.join(".solace");
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&outside, &link).is_ok();
        #[cfg(windows)]
        let linked = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .is_ok_and(|o| o.status.success());
        if linked {
            let err = worktree_add(&top, ".solace/worktrees/x", "b", "HEAD", false).unwrap_err();
            assert!(err.contains("is a link"), "{err}");
            assert!(remove_leftover(&top, &top.join(".solace/worktrees/x"), &[]).is_ok());
            assert!(
                fs::read_dir(&outside).unwrap().next().is_none(),
                "nothing written outside"
            );
            #[cfg(unix)]
            fs::remove_file(&link).unwrap();
            #[cfg(windows)]
            fs::remove_dir(&link).unwrap();
        }

        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        let pointer = |text: &str| {
            fs::write(sub.join(".git"), text).unwrap();
            gitdir_pointer(&sub)
        };
        assert!(pointer("gitdir: //server/share/x\n").is_none());
        assert!(pointer("gitdir: \\\\server\\share\\x\n").is_none());
        assert!(pointer("gitdir: /solace-no-such-ancestor/x\n").is_none());
        assert!(pointer("gitdir: ../real\n").is_some());
        let real = root.join("real");
        assert!(pointer(&format!("gitdir: {}\n", real.display())).is_some());
        fs::write(sub.join(".git"), "gitdir: //server/share/x\n").unwrap();
        assert!(external_git_dir(&sub).is_none(), "git is never run on it");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn toplevel_fails_outside_a_repository() {
        let dir = std::env::temp_dir().join(format!("solace-git-none-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let out = toplevel(&dir, false);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(out, Ok(None));
        assert_eq!(
            from_wsl_path("/mnt/c/Users/x"),
            Ok(PathBuf::from("C:/Users/x"))
        );
        assert!(from_wsl_path("/home/me/repo").is_err());
    }

    #[test]
    fn filters_from_the_repository_config_never_run() {
        if git(&std::env::temp_dir(), &["--version"]).is_err() {
            eprintln!("skipped: git is not installed");
            return;
        }
        let root = std::env::temp_dir().join(format!("solace-git-filter-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let run =
            |args: &[&str]| assert!(git(&root, args).unwrap().status.success(), "git {args:?}");
        run(&["init", "-q"]);
        fs::write(root.join("f.txt"), "a\n").unwrap();
        run(&["add", "f.txt"]);
        run(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "-m",
            "init",
        ]);
        // Plain git config writes, which bypass our overrides.
        for (key, value) in [
            ("filter.x.clean", "touch RAN_CLEAN; cat"),
            ("filter.x.process", "touch RAN_PROCESS"),
            ("filter.a=b.process", "touch RAN_ODD_NAME"),
        ] {
            let mut config = command(&root);
            assert!(
                config
                    .args(["config", key, value])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::write(
            root.join(".gitattributes"),
            "f.txt filter=x\n*.md filter=a=b\n",
        )
        .unwrap();
        fs::write(root.join("f.txt"), "a\nb\n").unwrap();
        fs::write(root.join("n.md"), "x\n").unwrap();

        status(&root).unwrap();
        diff(&root, "f.txt", None, false, false).unwrap();
        let ran: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|name| name.starts_with("RAN_"))
            .collect();
        let _ = fs::remove_dir_all(&root);
        assert!(ran.is_empty(), "filters ran: {ran:?}");
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
