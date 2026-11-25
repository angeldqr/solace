---
name: solace
description: Delegate work to other AI coding agents running beside you in Solace. Use when SOLACE_ENV=1 and a task splits into parts another agent (Codex, Claude Code, Gemini, ...) can do in parallel, or when you want a second agent to review your work.
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
  `claude`, `codex`, `gemini`, `qwen`, `kimi`, `opencode`, `copilot`, `cursor`, `amp`.
  Add `--down` to split below instead of to the right, or `--prompt "..."` to send a first task
  once the agent is ready.
- `agent prompt <name> "<text>" --wait` sends the text and waits until the agent stops working,
  then returns its final `state` and the visible `screen`. The wait gives up after 110 s by
  default (under your shell tool's usual 2-minute limit) with `"timedOut": true` and exit code 2;
  then run `solace agent wait <name>` again until it finishes.
- A `state` of `blocked` means the agent is waiting for a human (a permission or a question).
  Solace refuses to send prompts to it; tell the user which pane needs them.

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
