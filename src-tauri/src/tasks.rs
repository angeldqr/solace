//! Parallel tasks: `solace tasks start` turns a list of tasks into worker agents, each in its own
//! git worktree and pane, and keeps the board the UI shows. The rules live in solace-core.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Value, json};
use solace_core::agents::{AgentDef, AgentState, catalog};
use solace_core::tasks::{
    TaskSpec, TaskStatus, Tree, batch_id, branch, grid_plan, pick_kind, validate, worker_prompt,
};
use solace_core::terminals::PaneId;
use solace_infra::git;
use tauri::{AppHandle, Emitter, State};

use crate::agents::agent_in;
use crate::orchestration::{Launched, close, focused_pane, launch, split, wait_until, with_state};
use crate::usage::used_percent;
use crate::{AppState, Shared};

type Result<T = Value> = std::result::Result<T, String>;

/// A worker may sit on a folder-trust dialog for a while before the user gets to it.
const READY_WITHIN: Duration = Duration::from_secs(600);
const SUMMARY_MAX: usize = 4000;
/// A worker whose agent has been gone from its pane this long counts as closed.
const AGENT_GONE_AFTER: Duration = Duration::from_secs(20);
/// How long to wait for a closed pane's processes to let go of their worktree.
const EXIT_WITHIN: Duration = Duration::from_secs(5);

pub struct Batch {
    id: String,
    lead: Option<PaneId>,
    /// The repository's top folder; `None` when the workers share a plain folder.
    top: Option<PathBuf>,
    wsl: bool,
    base: Option<String>,
    created: u64,
    tasks: Vec<Task>,
}

pub struct Task {
    id: String,
    title: String,
    kind: &'static str,
    /// `None` when its pane could not be opened.
    pane: Option<PaneId>,
    branch: Option<String>,
    worktree: Option<PathBuf>,
    status: TaskStatus,
    summary: Option<String>,
    error: Option<String>,
    since: u64,
    /// When an agent was last seen in the pane.
    seen: Option<Instant>,
    /// Its agent was there and has been gone for a while (the pane is a plain shell now).
    gone: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    id: String,
    title: String,
    kind: &'static str,
    pane: Option<PaneId>,
    branch: Option<String>,
    worktree: Option<String>,
    status: TaskStatus,
    /// What the agent in the pane is doing right now; `None` when no agent runs there.
    agent_state: Option<AgentState>,
    summary: Option<String>,
    error: Option<String>,
    since: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchView {
    id: String,
    lead: Option<PaneId>,
    repo: Option<String>,
    base: Option<String>,
    created_at: u64,
    tasks: Vec<TaskView>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A task whose pane went away, or whose agent quit, before it reported reads as closed. An agent
/// started again in the pane brings it back.
fn status_of(s: &AppState, task: &Task) -> TaskStatus {
    let gone = task.gone || task.pane.is_none_or(|p| s.session.pane_root(p).is_none());
    match task.status {
        TaskStatus::Starting | TaskStatus::Running if gone => TaskStatus::Closed,
        status => status,
    }
}

/// Paths as shown to users and agents: one separator style.
fn shown(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn task_view(s: &AppState, t: &Task) -> TaskView {
    TaskView {
        id: t.id.clone(),
        title: t.title.clone(),
        kind: t.kind,
        pane: t.pane,
        branch: t.branch.clone(),
        worktree: t.worktree.as_deref().map(shown),
        status: status_of(s, t),
        agent_state: t.pane.and_then(|p| agent_in(s, p)).map(|(_, state)| state),
        summary: t.summary.clone(),
        error: t.error.clone().or_else(|| {
            (status_of(s, t) == TaskStatus::Closed && t.gone).then(|| "agent exited".into())
        }),
        since: t.since,
    }
}

/// Called every few hundred milliseconds: notes which task panes still run an agent. True when a
/// task just changed between running and "agent exited", so the UI must hear about it.
pub fn observe(s: &mut AppState) -> bool {
    if s.batches.is_empty() {
        return false;
    }
    let present: Vec<PaneId> = s
        .batches
        .iter()
        .flat_map(|b| &b.tasks)
        .filter_map(|t| t.pane)
        .filter(|p| agent_in(s, *p).is_some())
        .collect();
    let mut changed = false;
    for t in s.batches.iter_mut().flat_map(|b| b.tasks.iter_mut()) {
        let Some(pane) = t.pane else { continue };
        let here = present.contains(&pane);
        if here {
            t.seen = Some(Instant::now());
        }
        let gone = matches!(t.status, TaskStatus::Starting | TaskStatus::Running)
            && !here
            && t.seen.is_some_and(|at| at.elapsed() >= AGENT_GONE_AFTER);
        changed |= gone != t.gone;
        t.gone = gone;
    }
    changed
}

fn batch_view(s: &AppState, b: &Batch) -> BatchView {
    BatchView {
        id: b.id.clone(),
        lead: b.lead,
        repo: b.top.as_deref().map(shown),
        base: b.base.clone(),
        created_at: b.created,
        tasks: b.tasks.iter().map(|t| task_view(s, t)).collect(),
    }
}

pub fn views(s: &AppState) -> Vec<BatchView> {
    s.batches.iter().map(|b| batch_view(s, b)).collect()
}

fn emit(app: &AppHandle, s: &AppState) {
    let _ = app.emit("tasks", views(s));
}

/// Changes one task and tells the UI.
fn modify<T>(app: &AppHandle, task: &str, f: impl FnOnce(&mut Task) -> T) -> Option<T> {
    with_state(app, |s| {
        let found = s
            .batches
            .iter_mut()
            .flat_map(|b| b.tasks.iter_mut())
            .find(|t| t.id == task)
            .map(f);
        emit(app, s);
        found
    })
}

#[tauri::command]
pub fn tasks_get(state: State<Shared>) -> Vec<BatchView> {
    views(&state.lock().unwrap())
}

/// The "Clean up" button: closes the batch's panes and removes its worktrees, which git refuses
/// to do for one with uncommitted work.
#[tauri::command(async)]
pub fn tasks_clean(app: AppHandle, batch: String) -> Result<()> {
    let reply = clean(&app, &batch, false, None)?;
    let failed: Vec<String> = reply["failed"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|f| {
                    format!(
                        "{}: {}",
                        f["task"].as_str().unwrap_or_default(),
                        f["error"].as_str().unwrap_or_default()
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("\n"))
    }
}

/// The kind with the most room left among the agents that are installed and whose usage is read.
fn auto_kind(s: &AppState, lead: &'static str, wsl: bool) -> &'static str {
    let mut candidates: Vec<(&str, Option<f64>)> = Vec::new();
    for kind in ["claude", "codex"] {
        // Under WSL the agents live in the distro, where the Windows PATH can't tell.
        let installed = kind == lead
            || (!wsl
                && catalog()
                    .iter()
                    .find(|a| a.id == kind)
                    .and_then(AgentDef::command)
                    .is_some_and(solace_infra::system::on_path));
        if installed {
            candidates.push((kind, used_percent(s, kind)));
        }
    }
    if !candidates.iter().any(|c| c.0 == lead) {
        candidates.push((lead, used_percent(s, lead)));
    }
    let picked = pick_kind(lead, &candidates);
    candidates
        .iter()
        .find(|c| c.0 == picked)
        .map_or(lead, |c| c.0)
}

struct Context {
    lead: PaneId,
    root: PathBuf,
    wsl: bool,
    batch: String,
    kinds: Vec<&'static AgentDef>,
}

fn context(
    s: &AppState,
    from: Option<PaneId>,
    kind: Option<&str>,
    specs: &[TaskSpec],
) -> Result<Context> {
    let lead = from
        .or_else(|| focused_pane(s))
        .ok_or("no pane to start tasks from")?;
    if s.batches
        .iter()
        .flat_map(|b| &b.tasks)
        .any(|t| t.pane == Some(lead))
    {
        return Err("this pane is a task worker: workers don't start other tasks".into());
    }
    let root = s
        .session
        .pane_root(lead)
        .ok_or("unknown pane")?
        .to_path_buf();
    let lead_kind = agent_in(s, lead).map_or("claude", |(id, _)| id);
    let wsl = s
        .settings
        .shell
        .as_ref()
        .and_then(|p| p.file_stem())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("wsl"));
    let default = kind.unwrap_or(lead_kind);
    let mut kinds = Vec::new();
    for spec in specs {
        let want = spec.kind.as_deref().unwrap_or(default);
        let want = if want == "auto" {
            auto_kind(s, lead_kind, wsl)
        } else {
            want
        };
        let explicit = spec.kind.as_deref().or(kind).filter(|k| *k != "auto");
        let def = catalog()
            .iter()
            .find(|a| a.id == want && a.command().is_some())
            .ok_or_else(|| {
                let ids: Vec<&str> = catalog().iter().map(|a| a.id.as_str()).collect();
                format!(
                    "unknown agent kind {want:?}; use one of: auto, {}",
                    ids.join(", ")
                )
            })?;
        if explicit.is_some() && !wsl && !def.command().is_some_and(solace_infra::system::on_path) {
            return Err(format!(
                "{want} is not installed: its command {:?} is not on PATH",
                def.command().unwrap_or_default()
            ));
        }
        kinds.push(def);
    }
    let mut secs = now_ms() / 1000;
    let mut batch = batch_id(secs);
    while s.batches.iter().any(|b| b.id == batch) {
        secs += 1;
        batch = batch_id(secs);
    }
    Ok(Context {
        lead,
        root,
        wsl,
        batch,
        kinds,
    })
}

pub fn start(
    app: &AppHandle,
    from: Option<PaneId>,
    cwd: Option<PathBuf>,
    kind: Option<String>,
    shared: bool,
    specs: Vec<TaskSpec>,
) -> Result {
    validate(&specs)?;
    let ctx = with_state(app, |s| context(s, from, kind.as_deref(), &specs))?;

    let folder = cwd
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| ctx.root.clone());
    let text = folder.to_string_lossy();
    if text.starts_with(r"\\") && !text.starts_with(r"\\?\") {
        return Err("folders on the WSL file system (\\\\wsl$\\...) can't host tasks yet; open the folder by its Windows path".into());
    }
    let mut warnings: Vec<String> = Vec::new();
    let (top, base) = match git::toplevel(&folder, ctx.wsl)? {
        None if !shared => {
            return Err("this folder is not a git repository, so the workers would edit it all at the same time and step on each other's files. Run git init and make a first commit, or add --shared to accept that".into());
        }
        None => {
            warnings.push("not a git repository: the workers share this folder, so they may step on each other's files".into());
            (None, None)
        }
        Some(top) => {
            let base = git::head(&top, ctx.wsl)
                .map_err(|_| "commit at least once first: workers start from the latest commit")?;
            if git::is_dirty(&top, ctx.wsl).unwrap_or(false) {
                warnings.push(format!(
                    "workers start from HEAD {base}; your uncommitted changes aren't in their worktrees"
                ));
            }
            (Some(top), Some(base))
        }
    };

    // Git runs without the state lock: it can take a while on a big repository.
    let n = specs.len();
    let mut branches: Vec<Option<String>> = Vec::new();
    let mut worktrees: Vec<Option<PathBuf>> = Vec::new();
    if let Some(top) = &top {
        for (i, spec) in specs.iter().enumerate() {
            let name = branch(&ctx.batch, i + 1, &spec.title);
            let rel = format!(".solace/worktrees/{}-{}", ctx.batch, i + 1);
            if let Err(err) = git::worktree_add(top, &rel, &name, ctx.wsl) {
                // No pane exists yet, so undoing the ones made is all there is to do. The failed
                // one may have made its branch before failing.
                let _ = git::branch_delete(top, &name, ctx.wsl);
                for (rel, name) in worktrees.iter().flatten().zip(branches.iter().flatten()) {
                    let rel = rel_of(top, rel).unwrap_or_default();
                    let _ = git::worktree_remove(top, &rel, true, ctx.wsl);
                    let _ = git::branch_delete(top, name, ctx.wsl);
                }
                return Err(format!(
                    "could not create the worktree for task {}: {err}",
                    i + 1
                ));
            }
            worktrees.push(Some(top.join(&rel)));
            branches.push(Some(name));
        }
    } else {
        branches.resize(n, None);
        worktrees.resize(n, None);
    }

    let mut panes: Vec<Option<PaneId>> = vec![Some(ctx.lead)];
    let mut tasks: Vec<Task> = Vec::new();
    for (i, (at, direction)) in grid_plan(n).into_iter().enumerate() {
        let id = format!("{}-{}", ctx.batch, i + 1);
        let place = worktrees[i].as_deref().unwrap_or(&folder);
        let parent = panes[at].or(Some(ctx.lead));
        let opened = split(
            app,
            parent,
            direction,
            Some(&id),
            Some(place),
            &[("SOLACE_TASK_ID", id.clone())],
        );
        let (pane, error) = match opened {
            Ok(pane) => (Some(pane), None),
            Err(err) => (None, Some(err)),
        };
        panes.push(pane);
        tasks.push(Task {
            id,
            title: specs[i].title.trim().to_string(),
            kind: ctx.kinds[i].id.as_str(),
            pane,
            branch: branches[i].clone(),
            worktree: worktrees[i].clone(),
            status: if error.is_some() {
                TaskStatus::Failed
            } else {
                TaskStatus::Starting
            },
            summary: None,
            error,
            since: now_ms(),
            seen: None,
            gone: false,
        });
    }

    let reply = json!({
        "batch": ctx.batch,
        "base": base,
        "warnings": warnings,
        "tasks": tasks.iter().map(|t| json!({
            "id": t.id, "title": t.title, "kind": t.kind, "pane": t.pane,
            "branch": t.branch, "worktree": t.worktree.as_deref().map(shown), "error": t.error,
        })).collect::<Vec<_>>(),
    });
    let top_text = top.as_deref().map(shown);
    let prompts: Vec<(String, Option<PaneId>, String)> = tasks
        .iter()
        .zip(&specs)
        .map(|(t, spec)| {
            let folder = t.worktree.as_deref().map(shown);
            let tree = match (&t.branch, &folder, &top_text) {
                (Some(branch), Some(folder), Some(top)) => Some(Tree {
                    branch,
                    folder,
                    top,
                }),
                _ => None,
            };
            (t.id.clone(), t.pane, worker_prompt(&t.id, spec, tree))
        })
        .collect();
    with_state(app, |s| {
        s.batches.push(Batch {
            id: ctx.batch.clone(),
            lead: Some(ctx.lead),
            top,
            wsl: ctx.wsl,
            base,
            created: now_ms(),
            tasks,
        });
        emit(app, s);
    });

    // Each worker starts on its own thread: a folder-trust dialog in one must not hold up the rest.
    for ((id, pane, prompt), def) in prompts.into_iter().zip(ctx.kinds) {
        let Some(pane) = pane else { continue };
        let app = app.clone();
        thread::spawn(move || {
            // Only while the task still waits for its prompt: a task that finished, failed or
            // was cleaned up must never get anything typed.
            let live = |s: &AppState| {
                s.batches
                    .iter()
                    .flat_map(|b| &b.tasks)
                    .any(|t| t.id == id && t.pane == Some(pane) && t.status == TaskStatus::Starting)
            };
            let outcome = launch(&app, pane, def, &prompt, READY_WITHIN, true, &live);
            // A pane the user closed meanwhile leaves the task reading as closed, not failed.
            if matches!(outcome, Ok(Launched::Gone))
                || with_state(&app, |s| s.session.pane_root(pane).is_none())
            {
                return;
            }
            modify(&app, &id, |t| {
                // A worker that reported before the prompt call returned keeps its result.
                if t.status != TaskStatus::Starting {
                    return;
                }
                t.since = now_ms();
                match outcome {
                    Ok(Launched::Sent) => t.status = TaskStatus::Running,
                    Ok(Launched::Blocked | Launched::TimedOut | Launched::Gone) => {
                        t.status = TaskStatus::Failed;
                        t.error = Some("the agent did not become ready".into());
                    }
                    Err(err) => {
                        t.status = TaskStatus::Failed;
                        t.error = Some(err);
                    }
                }
            });
        });
    }
    Ok(reply)
}

fn rel_of(top: &Path, worktree: &Path) -> Option<String> {
    let rel = worktree.strip_prefix(top).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// A worker's report. Its work must be committed: the lead merges from the branch.
pub fn done(
    app: &AppHandle,
    task: &str,
    pane: Option<PaneId>,
    summary: &str,
    failed: bool,
) -> Result {
    let (worktree, branch, top, base, wsl) = with_state(app, |s| {
        let (batch, found) = s
            .batches
            .iter()
            .find_map(|b| Some((b, b.tasks.iter().find(|t| t.id == task)?)))
            .ok_or_else(|| format!("unknown task {task:?}"))?;
        if pane.is_some() && pane != found.pane {
            return Err(format!("task {task} belongs to another pane"));
        }
        Ok((
            found.worktree.clone(),
            found.branch.clone(),
            batch.top.clone(),
            batch.base.clone(),
            batch.wsl,
        ))
    })?;
    // If git itself fails the report is let through: losing it would be worse.
    if !failed && let Some(dir) = &worktree {
        let name = branch.as_deref().unwrap_or("your branch");
        if git::is_dirty(dir, wsl).unwrap_or(false) {
            return Err(format!(
                "you have uncommitted changes: commit them on {name} first, then run solace task done again"
            ));
        }
        if let (Some(top), Some(base), Some(branch)) = (&top, &base, &branch)
            && git::commits_since(top, base, branch, wsl) == Ok(0)
        {
            return Err(format!(
                "no commits on {name}: commit your work or use --failed"
            ));
        }
    }
    let summary: String = summary.chars().take(SUMMARY_MAX).collect();
    modify(app, task, |t| {
        t.status = if failed {
            TaskStatus::Failed
        } else {
            TaskStatus::Done
        };
        t.summary = Some(summary);
        t.error = failed.then(|| "the agent reported it could not finish".to_string());
        t.since = now_ms();
    });
    Ok(json!({ "task": task, "status": if failed { "failed" } else { "done" } }))
}

pub fn wait(
    app: &AppHandle,
    batch: Option<String>,
    from: Option<PaneId>,
    timeout: Duration,
) -> Result {
    let id = with_state(app, |s| match batch {
        Some(id) if s.batches.iter().any(|b| b.id == id) => Ok(id),
        Some(id) => Err(format!("no batch {id:?}")),
        None => s
            .batches
            .iter()
            .rev()
            .find(|b| from.is_some() && b.lead == from)
            .map(|b| b.id.clone())
            .ok_or_else(|| "no tasks were started from this pane".to_string()),
    })?;
    // A batch cleaned up meanwhile counts as finished.
    let finished = wait_until(timeout, || {
        with_state(app, |s| {
            s.batches
                .iter()
                .find(|b| b.id == id)
                .is_none_or(|b| b.tasks.iter().all(|t| status_of(s, t).finished()))
        })
    });
    with_state(app, |s| {
        let batch = s
            .batches
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| format!("batch {id:?} was cleaned up"))?;
        let view = batch_view(s, batch);
        Ok(json!({
            "batch": id, "base": view.base, "timedOut": !finished, "tasks": view.tasks,
        }))
    })
}

/// Closes the batch's panes and removes its worktrees. Without `force` git refuses a worktree
/// with uncommitted changes, which is the guard against losing work. Branches are never deleted.
/// The panes a clean-up may close: those still named after their task (so the pane really is the
/// one the task created), and never the lead or the pane that asked.
fn closable(
    tasks: &[(String, Option<PaneId>)],
    names: &std::collections::HashMap<PaneId, String>,
    protected: &[Option<PaneId>],
) -> Vec<PaneId> {
    tasks
        .iter()
        .filter_map(|(id, pane)| {
            let pane = (*pane)?;
            (names.get(&pane) == Some(id) && !protected.contains(&Some(pane))).then_some(pane)
        })
        .collect()
}

pub fn clean(app: &AppHandle, batch: &str, force: bool, from: Option<PaneId>) -> Result {
    let (top, wsl, items, panes) = with_state(app, |s| {
        let b = s
            .batches
            .iter()
            .find(|b| b.id == batch)
            .ok_or_else(|| format!("no batch {batch:?}"))?;
        if !force
            && let Some(t) = b
                .tasks
                .iter()
                .find(|t| matches!(status_of(s, t), TaskStatus::Starting | TaskStatus::Running))
        {
            return Err(format!(
                "task {} is still running: wait for it with solace tasks wait",
                t.id
            ));
        }
        let items: Vec<(String, Option<PaneId>, Option<String>)> = b
            .tasks
            .iter()
            .map(|t| {
                let rel = b
                    .top
                    .as_deref()
                    .zip(t.worktree.as_deref())
                    .and_then(|(top, wt)| rel_of(top, wt));
                (t.id.clone(), t.pane, rel)
            })
            .collect();
        let owned: Vec<(String, Option<PaneId>)> =
            b.tasks.iter().map(|t| (t.id.clone(), t.pane)).collect();
        let panes = closable(&owned, &s.names, &[b.lead, from]);
        Ok::<_, String>((b.top.clone(), b.wsl, items, panes))
    })?;
    // Before anything is closed: a worktree with unsaved work must not lose its pane either.
    if !force && let Some(top) = &top {
        for (id, _, rel) in &items {
            if let Some(rel) = rel
                && git::is_dirty(&top.join(rel), wsl).unwrap_or(false)
            {
                return Err(format!(
                    "task {id} has uncommitted changes in its worktree: commit or discard them first"
                ));
            }
        }
    }
    let pids: Vec<u32> = with_state(app, |s| {
        panes.iter().filter_map(|p| s.ptys.get(p)?.pid()).collect()
    });
    for pane in panes {
        close(app, pane);
    }
    // Windows won't delete a folder while a process still holds it.
    wait_until(EXIT_WITHIN, || {
        pids.iter()
            .all(|pid| !solace_infra::system::process_alive(*pid))
    });
    let (mut removed, mut failed) = (Vec::new(), Vec::new());
    for (id, _, rel) in items {
        match (&top, rel) {
            (Some(top), Some(rel)) => match git::worktree_remove_retrying(top, &rel, force, wsl) {
                Ok(()) => removed.push(id),
                Err(err) => failed.push(json!({ "task": id, "error": err })),
            },
            _ => removed.push(id),
        }
    }
    with_state(app, |s| {
        if failed.is_empty() {
            s.batches.retain(|b| b.id != batch);
        } else if let Some(b) = s.batches.iter_mut().find(|b| b.id == batch) {
            // A second try must not go after the worktrees that are already gone.
            for t in b.tasks.iter_mut().filter(|t| removed.contains(&t.id)) {
                t.worktree = None;
            }
        }
        emit(app, s);
    });
    Ok(json!({ "batch": batch, "removed": removed, "failed": failed }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn clean_up_closes_only_the_panes_its_tasks_created() {
        let tasks = vec![
            ("b1-1".to_string(), Some(PaneId(12))),
            ("b1-2".to_string(), Some(PaneId(13))),
            ("b1-3".to_string(), Some(PaneId(14))),
            ("b1-4".to_string(), None),
        ];
        let names: HashMap<PaneId, String> = [
            (PaneId(12), "b1-1".to_string()),
            (PaneId(13), "other".to_string()), // the id now belongs to something else
            (PaneId(14), "b1-3".to_string()),
        ]
        .into();
        // 14 is also the caller: never closed.
        let closed = closable(&tasks, &names, &[Some(PaneId(1)), Some(PaneId(14))]);
        assert_eq!(closed, vec![PaneId(12)]);
    }
}
