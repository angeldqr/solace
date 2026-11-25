use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;

use ignore::WalkBuilder;
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    /// Matched by a .gitignore: still listed (build output is worth seeing) but shown dimmed.
    pub ignored: bool,
}

/// One directory level, folders first. Listing lazily keeps huge repositories instant.
pub fn list(dir: &Path) -> io::Result<Vec<Entry>> {
    let visible: HashSet<OsString> = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(false)
        // Only git's rules: .ignore files are for search tools.
        .ignore(false)
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.depth() == 1)
        .map(|entry| entry.file_name().to_owned())
        .collect();
    let mut entries: Vec<Entry> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != ".git")
        .map(|entry| Entry {
            ignored: !visible.contains(&entry.file_name()),
            // Links are not followed: one pointing at a network share would make Windows sign in
            // to that host just to list this folder.
            dir: entry.file_type().is_ok_and(|t| t.is_dir()),
            name: entry.file_name().to_string_lossy().into_owned(),
        })
        .collect();
    entries.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_folders_first_and_marks_gitignored_entries() {
        let root = std::env::temp_dir().join(format!("solace-files-test-{}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        fs::write(root.join("b.txt"), "").unwrap();
        fs::write(root.join("A.md"), "").unwrap();
        fs::write(root.join("debug.log"), "").unwrap();

        let names: Vec<(String, bool, bool)> = list(&root)
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.dir, e.ignored))
            .collect();
        let expect = |n: &str, d, i| (n.to_string(), d, i);
        assert_eq!(
            names,
            vec![
                expect("src", true, false),
                expect("target", true, true),
                expect(".gitignore", false, false),
                expect("A.md", false, false),
                expect("b.txt", false, false),
                expect("debug.log", false, true),
            ]
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
