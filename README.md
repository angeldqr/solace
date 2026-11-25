# Solace

A desktop terminal for running and orchestrating AI coding agents: Claude Code, Codex, Gemini, Kimi, or any CLI agent. Agents can start other agents and hand them work, all inside one workspace.

> Status: early development (v0.0.x). Not ready for daily use yet.

## Features

- **Terminals** with tabs and resizable splits, grouped into spaces (one per project folder). Pick PowerShell, cmd, Git Bash or WSL as the shell.
- **Agent awareness**: Claude Code, Codex, Gemini CLI, Qwen Code, Kimi Code, OpenCode, Copilot, Cursor Agent and Amp are recognized in any pane, with their state (working, needs you, done, idle).
- **Orchestration**: inside Solace, the `solace` CLI lets an agent open panes, start other agents, prompt them and read their answers (`solace agent start reviewer --kind codex`, `solace agent prompt reviewer "..." --wait`). Run `solace skill install` to teach Claude Code how.
- **Files and Changes** panels with a lazy file tree, git status and diffs, refreshed live.
- **Themes**: five built-in themes (one light), VS Code theme import, terminal font and cursor settings, English and Spanish.
- **Updates** download in the background and apply when you choose to restart.

## Stack

- **Core:** Rust. PTYs, agent detection and orchestration.
- **Shell:** [Tauri 2](https://tauri.app), so the native window ships as a ~2 MB installer.
- **UI:** React 19, Tailwind CSS v4, Lucide icons, Inter.

## Layout

```
crates/solace-core   Domain and use cases. Pure Rust, no IO.
src-tauri            Desktop app (composition root, Tauri commands)
src                  React UI, organised by feature
```

## Development

Requirements: Rust (stable, MSVC on Windows), Node 24+, pnpm 10, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm tauri dev      # run the app
cargo test --workspace
pnpm lint
```

## Releases

Pushing a `vX.Y.Z` tag that matches `version` in `Cargo.toml` builds the Windows installers (`.exe` and `.msi`) and signed update bundles, and attaches them to a draft GitHub release. Publishing the draft makes installed copies download the update and offer a restart.

Signing needs the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The public key lives in `src-tauri/tauri.conf.json`; losing the private key means installed copies can no longer update.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org) (pending approval; until then releases are unsigned).

- Committers and reviewers: [maintainers of this repository](https://github.com/angeldqr/solace/graphs/contributors)
- Approvers: [repository owner](https://github.com/angeldqr)

Setup details: [docs/code-signing.md](docs/code-signing.md).

## Privacy

Solace does not collect or send any usage data. The only network requests it makes on its own are to GitHub Releases, to check for and download updates. You can turn this off in **Settings > About**. Agents and programs you run in its terminals follow their own privacy policies.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attributions.
