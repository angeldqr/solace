---
name: solace
description: Delegate work to other AI coding agents running beside you in Solace. Use when SOLACE_ENV=1 and a task splits into parts another agent (Codex, Claude Code, Antigravity, ...) can do in parallel, or when you want a second agent to review your work, or when the user gives several independent tasks: fan them out in parallel with `solace tasks start`.
---

# Solace: work with other agents

You are running inside a Solace terminal (`SOLACE_ENV=1`). The `solace` command controls the
workspace around you: it opens panes next to yours, starts other agents in them, sends them
prompts and reads their screens. Every command prints JSON.

## Start an agent and give it a task

```sh
solace agent start reviewer --kind codex
solace agent prompt reviewer "Review the diff in src/auth for bugs. Reply with a short list." --wait
```

- `agent start <name> --kind <kind>` splits your pane and launches the agent there. Kinds:
  `claude`, `codex`, `antigravity`, `qwen`, `kimi`, `opencode`, `copilot`, `cursor`, `amp`.
  Add `--down` to split below instead of to the right, or `--prompt "..."` to send a first task
  once the agent is ready.
- `agent prompt <name> "<text>" --wait` sends the text and waits until the agent stops working,
  then returns its final `state` and the visible `screen`. The wait gives up after 110 s by
  default (under your shell tool's usual 2-minute limit) with `"timedOut": true` and exit code 2;
  then run `solace agent wait <name>` again until it finishes.
- A `state` of `blocked` means the agent is waiting for a human (a permission or a question).
  Solace refuses to send prompts to it; tell the user which pane needs them.

## Several independent tasks

When the user hands you two or more tasks that don't depend on each other ("fix these 4 bugs",
"do A, B and C"), don't do them one after another: start one worker agent per task with a single
command. Each worker gets its own git worktree and branch plus its own pane, looks for skills that
help before it starts, and reports back when done. Use it for 2 to 8 tasks; do a single task
yourself.

Give a JSON array of `{"title", "prompt", "kind"?}`. Each prompt must stand alone: the worker
knows nothing of your conversation, so name the files, the problem and the expected result. Write
paths relative to the repository (`src/auth/login.ts`), never absolute: a worker's folder is a
worktree, not the folder you are in.

```sh
solace tasks start <<'EOF'
[
  {"title": "Fix login redirect", "prompt": "In src/auth/login.ts the redirect after login ignores ?next=. Fix it and add a test."},
  {"title": "Dark mode contrast", "prompt": "The sidebar text in dark mode fails WCAG AA. Adjust the tokens in src/theme.css.", "kind": "codex"}
]
EOF
```

On Windows PowerShell write the JSON to a file with a single-quoted here-string (piped
non-ASCII text gets mangled there):

```powershell
@'
[{"title":"Fix login","prompt":"In src/auth/login.ts ..."}]
'@ | Set-Content -Encoding utf8 $env:TEMP\tasks.json
solace tasks start --file $env:TEMP\tasks.json
```

- `--kind <kind>` sets the agent for tasks that name none (default: your own kind); `--kind auto`
  picks the one with the most room left in its usage limits. Mix kinds per task, e.g. `codex` for
  a second opinion. A kind that isn't installed is an error.
- The folder must be a git repository with at least one commit; workers start from the last
  commit, so your uncommitted changes are not in their worktrees (you get a warning). In a
  folder that isn't a repository, `--shared` lets all workers edit it together; only do that if
  the user agrees.
- The reply lists each task's `id`, `branch` and `worktree`, plus warnings.
  `solace pane read <id>` shows a worker's screen.
- Then wait: `solace tasks wait` blocks until all are finished and returns each task's status,
  summary and branch. It gives up after 110 s with `"timedOut": true` and exit code 2; run it
  again until it finishes. `solace tasks list` shows the state without waiting (exit code 0).
- Each worker runs in a new folder, so its agent may ask the user to trust that folder the first
  time. The worker starts by itself once they answer. Tell the user which panes (task ids and
  titles) need that, so nothing looks stuck. A worker whose agent quits shows as `closed` with
  the error `agent exited`; it goes back to running if the user starts the agent again in that pane.
- If a task's `agentState` is `blocked`, its agent is waiting for the user (a permission or a
  question): don't keep waiting. Tell the user which task (id and title) needs them and in which
  pane, then wait again once they have answered.
- Review each branch (`git log`, `git diff <base>..<branch>`), tell the user what each worker did
  and which skills they suggest (workers never install anything), and merge only when the user
  asks. Then `solace tasks clean <batch>` closes the panes and removes the worktrees. It refuses
  while a task still runs or a worktree has uncommitted work. Never add `--force` unless the user
  explicitly asks for it: it throws that work away. Branches are always kept.
- Worktrees live in `.solace/` inside the repository, which has its own `.gitignore`. Tools that
  ignore `.gitignore` (some test runners, bundlers, searches) may need `.solace` excluded.

## Review round (agents reviewing each other)

Once `solace tasks wait` shows every worker done, start a second batch where each task reviews
another worker's branch, preferably with a different CLI (codex reviews claude's branch and vice
versa). `base` makes the reviewer's worktree start from that branch, so it sees the work. Use the
base sha from the first batch's reply.

```sh
solace tasks start <<'EOF'
[
  {"title": "Review: Fix login redirect", "kind": "codex", "base": "solace/b1x-1-fix-login-redirect",
   "prompt": "Review the commits on this branch since <batch base sha> (git diff <sha>..HEAD) for bugs, security and missing tests. Fix real problems with commits; then report findings with solace task done."}
]
EOF
```

A reviewer that finds nothing to fix reports with `solace task done --no-changes --summary '...'`
(no commits needed; uncommitted changes are still refused). Then summarise the findings for the user;
merge only when they ask.

## If SOLACE_TASK_ID is set, you are a worker

Another agent started you for one task. Do only that task, in your folder (edit only inside it). If
there is a branch, commit your work on it: `task done` refuses while there are uncommitted changes
or no commits. Then run
`solace task done --summary '<what you did, files, how to verify>'` (single quotes in bash; for a
long summary or one with quotes, write it to a file and use `--summary-file <path>`). Add
`--failed` and explain if you could not finish. Never start other agents or tasks.

## Other commands

```sh
solace agent list                 # every agent: pane, name, kind, state
solace agent wait <name>          # wait until it is idle or blocked
solace pane read <name|id>        # the screen as text (--lines N for the last N lines)
solace pane send <name|id> "ls"   # type into a pane and press Enter (--no-enter to skip it)
solace pane split [--down]        # an empty shell next to yours
solace pane close <name|id>
solace pane list
```

## Good practice

- Give each agent one self-contained task with the files and the expected output format.
- Run independent tasks in parallel: start several agents, prompt them without `--wait`, then
  `agent wait` on each.
- Read results with `pane read` instead of assuming them, and close panes you no longer need.
- `pane send` types into a real shell, so it runs commands like anything you type. Don't ask the
  user to auto-approve `solace` commands without that in mind.
