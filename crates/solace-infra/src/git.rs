use std::fs;
use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

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
