//! Parallel tasks: the pure rules for fanning a list of tasks out to worker agents, one git
//! worktree and one pane each.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::terminals::Direction;

/// Most workers one batch may start; more would make panes too small to read.
pub const MAX_TASKS: usize = 8;

/// One task from the lead agent: what to call it and what to tell the worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskSpec {
    pub title: String,
    pub prompt: String,
    /// Agent kind for this task; the batch's kind when absent.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Starting,
    Running,
    Done,
    Failed,
    /// Its pane went away before the worker reported.
    Closed,
}

impl TaskStatus {
    /// Nothing more will happen to the task.
    pub fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Closed)
    }
}

pub fn validate(tasks: &[TaskSpec]) -> Result<(), String> {
    if tasks.is_empty() {
        return Err("no tasks given".into());
    }
    if tasks.len() > MAX_TASKS {
        return Err(format!(
            "at most {MAX_TASKS} tasks at a time ({} given); start the rest in a second batch",
            tasks.len()
        ));
    }
    for (i, task) in tasks.iter().enumerate() {
        if task.title.trim().is_empty() || task.prompt.trim().is_empty() {
            return Err(format!("task {} needs a title and a prompt", i + 1));
        }
    }
    Ok(())
}

/// Lowercase ASCII words joined by '-', for branch names; accents are folded to their letter.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in title.chars() {
        let c = fold(c).to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out.truncate(30);
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "task".into()
    } else {
        out.into()
    }
}

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        c => c,
    }
}

/// "b" and the time in base 36: starts with a letter, so it can name an agent.
pub fn batch_id(unix_secs: u64) -> String {
    let mut n = unix_secs;
    let mut digits = Vec::new();
    while n > 0 {
        digits.push(char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    let tail: String = digits.iter().rev().collect();
    format!("b{tail}")
}

pub fn branch(batch: &str, n: usize, title: &str) -> String {
    format!("solace/{batch}-{n}-{}", slug(title))
}

/// Where each worker's pane goes: (index of the pane to split, direction). Index 0 is the lead,
/// worker k is index k. The first worker takes the right half; the next ones split the oldest
/// worker pane first, alternating down and right, so four workers make a 2x2 grid.
pub fn grid_plan(n: usize) -> Vec<(usize, Direction)> {
    let mut plan = Vec::new();
    // Worker panes that can still be split, oldest first, with how many times each was.
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    for worker in 1..=n {
        if let Some((pane, depth)) = queue.pop_front() {
            let direction = if depth % 2 == 0 {
                Direction::Column
            } else {
                Direction::Row
            };
            plan.push((pane, direction));
            queue.push_back((pane, depth + 1));
            queue.push_back((worker, depth + 1));
        } else {
            plan.push((0, Direction::Row));
            queue.push_back((worker, 0));
        }
    }
    plan
}

/// Where a worker's git worktree is: its branch, its folder and the repository's top folder
/// (all paths with forward slashes, as shown to the worker).
pub struct Tree<'a> {
    pub branch: &'a str,
    pub folder: &'a str,
    pub top: &'a str,
}

pub fn worker_prompt(id: &str, spec: &TaskSpec, tree: Option<Tree>) -> String {
    let place = match tree {
        Some(Tree { branch, folder, top }) => format!(
            "You work in your own git worktree on branch `{branch}`. Your folder is {folder}. Edit only inside it. \
             If the task mentions paths under {top}, use the same relative path inside your folder. \
             Commit your work there before you report. Don't push, merge or switch branches."
        ),
        None => "Other agents edit this folder at the same time. Change only what your task needs and don't commit.".into(),
    };
    format!(
        "You are Solace task {id}: \"{title}\". {place}\n\
         Before starting, check which skills can help: use relevant installed skills (~/.claude/skills, .claude/skills, ~/.agents/skills, .agents/skills). \
         You may search more with `npx -y skills find <keywords>`, but DO NOT install anything: list useful ones in your summary with their `npx skills add <source>` command.\n\
         Don't start other agents or Solace tasks.\n\
         When finished run: solace task done --summary '<what you did, files, how to verify>' (for a long summary or one with single quotes, write it to a file and use --summary-file <path>). \
         If you can't finish: add --failed and explain.\n\n\
         Task:\n{prompt}",
        title = spec.title,
        prompt = spec.prompt,
    )
}

/// The kind with the most room left: lowest highest-used percentage, unknown counting as 50.
/// A tie goes to the lead's own kind, then to the first listed.
pub fn pick_kind(lead: &str, candidates: &[(&str, Option<f64>)]) -> String {
    let score = |c: &(&str, Option<f64>)| c.1.unwrap_or(50.0);
    let best = candidates.iter().map(score).fold(f64::INFINITY, f64::min);
    candidates
        .iter()
        .filter(|c| score(c) == best)
        .map(|c| c.0)
        .find(|kind| *kind == lead)
        .or_else(|| candidates.iter().find(|c| score(c) == best).map(|c| c.0))
        .unwrap_or(lead)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(title: &str, prompt: &str) -> TaskSpec {
        TaskSpec {
            title: title.into(),
            prompt: prompt.into(),
            kind: None,
        }
    }

    #[test]
    fn slugs_are_plain_ascii_words() {
        assert_eq!(
            slug("Corregir el botón de Añadir"),
            "corregir-el-boton-de-anadir"
        );
        assert_eq!(slug("fix 🐛 crash!!"), "fix-crash");
        assert_eq!(slug(""), "task");
        assert_eq!(slug("🐛🐛"), "task");
        let long = slug(&"palabra ".repeat(20));
        assert!(long.len() <= 30 && !long.ends_with('-'), "{long}");
    }

    #[test]
    fn branches_are_valid_git_refs() {
        for title in ["Fix  the..bug.lock", "a b", "ñandú", ".lock", "x/y"] {
            let b = branch("b1x", 2, title);
            assert!(b.starts_with("solace/b1x-2-"), "{b}");
            assert!(
                !b.contains(' ') && !b.contains("..") && !b.ends_with(".lock"),
                "{b}"
            );
        }
    }

    #[test]
    fn batch_ids_start_with_a_letter() {
        for secs in [0, 1, 1_700_000_000, u64::MAX] {
            let id = batch_id(secs);
            assert!(id.chars().next().unwrap().is_ascii_alphabetic(), "{id}");
            assert!(id.parse::<u32>().is_err());
        }
        assert_eq!(batch_id(36), "b10");
    }

    #[test]
    fn four_workers_make_a_two_by_two_grid() {
        assert_eq!(
            grid_plan(4),
            vec![
                (0, Direction::Row),
                (1, Direction::Column),
                (1, Direction::Row),
                (2, Direction::Row),
            ]
        );
        for n in 1..=MAX_TASKS {
            let plan = grid_plan(n);
            assert_eq!(plan.len(), n);
            for (worker, (at, _)) in plan.iter().enumerate() {
                assert!(
                    *at <= worker,
                    "worker {} splits a pane that doesn't exist yet",
                    worker + 1
                );
            }
            // A pane that is split again turns the other way, so the grid stays square-ish.
            for pane in 0..=n {
                let turns: Vec<_> = plan
                    .iter()
                    .filter(|(at, _)| *at == pane)
                    .map(|p| p.1)
                    .collect();
                assert!(
                    turns.windows(2).all(|w| w[0] != w[1]),
                    "pane {pane}: {turns:?}"
                );
            }
        }
    }

    #[test]
    fn validates_counts_and_fields() {
        assert!(validate(&[]).is_err());
        assert!(validate(&vec![spec("a", "b"); MAX_TASKS + 1]).is_err());
        assert!(validate(&vec![spec("a", "b"); MAX_TASKS]).is_ok());
        assert!(validate(&[spec("a", "  ")]).is_err());
        assert!(validate(&[spec(" ", "b")]).is_err());
    }

    #[test]
    fn a_task_without_kind_parses() {
        let t: TaskSpec = serde_json::from_str(r#"{"title":"t","prompt":"p"}"#).unwrap();
        assert_eq!(t.kind, None);
        let t: TaskSpec =
            serde_json::from_str(r#"{"title":"t","prompt":"p","kind":"codex"}"#).unwrap();
        assert_eq!(t.kind.as_deref(), Some("codex"));
    }

    #[test]
    fn picks_the_kind_with_most_room() {
        assert_eq!(
            pick_kind("claude", &[("claude", Some(80.0)), ("codex", Some(10.0))]),
            "codex"
        );
        // Ties and unknowns go to the lead.
        assert_eq!(
            pick_kind("codex", &[("claude", Some(20.0)), ("codex", Some(20.0))]),
            "codex"
        );
        assert_eq!(
            pick_kind("claude", &[("claude", None), ("codex", Some(50.0))]),
            "claude"
        );
        assert_eq!(
            pick_kind("gemini", &[("gemini", None), ("codex", Some(90.0))]),
            "gemini"
        );
        // Everything exhausted: still an answer, the lead's.
        assert_eq!(
            pick_kind("claude", &[("claude", Some(100.0)), ("codex", Some(100.0))]),
            "claude"
        );
        assert_eq!(pick_kind("claude", &[]), "claude");
    }

    #[test]
    fn worker_prompts_carry_the_essentials() {
        let tree = Tree {
            branch: "solace/b1x-1-fix-it",
            folder: "C:/repo/.solace/worktrees/b1x-1",
            top: "C:/repo",
        };
        let p = worker_prompt("b1x-1", &spec("Fix it", "do the thing"), Some(tree));
        for needle in [
            "b1x-1",
            "`solace/b1x-1-fix-it`",
            "Your folder is C:/repo/.solace/worktrees/b1x-1. Edit only inside it.",
            "paths under C:/repo, use the same relative path",
            "npx -y skills find",
            "do the thing",
            "solace task done --summary '",
            "--summary-file",
        ] {
            assert!(p.contains(needle), "{needle} missing from {p}");
        }
        assert!(!p.contains('$'));
        assert!(p.to_lowercase().contains("do not install"));
        let shared = worker_prompt("b1x-2", &spec("t", "p"), None);
        assert!(shared.contains("Other agents edit this folder") && !shared.contains("worktree"));
    }
}
