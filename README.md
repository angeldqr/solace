# Solace

A desktop terminal for running and orchestrating AI coding agents: Claude Code, Codex, Gemini, Kimi, or any CLI agent. Agents can start other agents and hand them work, all inside one workspace.

> Status: early development (v0.0.x). Not ready for daily use yet.

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

Pushing a `vX.Y.Z` tag that matches `version` in `Cargo.toml` builds the Windows installers (`.exe` and `.msi`) and attaches them to a draft GitHub release.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attributions.
