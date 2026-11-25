//! Installs Solace's integration into Claude Code: hooks in `~/.claude/settings.json` that report
//! the agent's exact state, and the skill that teaches it the `solace` CLI.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use solace_core::agents::hooks::CLAUDE_EVENTS;

const SKILL: &str = include_str!("../../../skills/solace/SKILL.md");
/// Marks hook commands as Solace's, so updates and uninstalls touch nothing else.
const MARKER: &str = "solace-hook";

pub struct ClaudePaths {
    pub settings: PathBuf,
    pub skill: PathBuf,
}

impl ClaudePaths {
    pub fn in_home(home: &Path) -> Self {
        let claude = home.join(".claude");
        ClaudePaths {
            settings: claude.join("settings.json"),
            skill: claude.join("skills").join("solace").join("SKILL.md"),
        }
    }
}

/// The command Claude Code runs (in Git Bash on Windows) for every listened event. The `|| true`
/// keeps any failure, such as Solace being uninstalled, from blocking Claude.
fn hook_command(solace_exe: &Path) -> String {
    let path = solace_exe.to_string_lossy();
    // Tauri's paths are verbatim (`\\?\C:\...`), which bash can't run; forward slashes it can.
    let path = path
        .strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .replace('\\', "/");
    format!("\"{path}\" hook --source {MARKER} 2>/dev/null || true")
}

fn is_ours(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.contains(MARKER))
}

/// Reads settings.json; a file that exists but isn't JSON is an error, never overwritten.
fn read_settings(path: &Path) -> io::Result<Map<String, Value>> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Map::new()),
        Ok(text) => match serde_json::from_str(text.trim_start_matches('\u{feff}'))? {
            Value::Object(map) => Ok(map),
            _ => Err(io::Error::other("settings.json is not a JSON object")),
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
        Err(err) => Err(err),
    }
}

/// Removes Solace's hooks, dropping matcher groups and events left empty.
fn strip_ours(settings: &mut Map<String, Value>) {
    let Some(Value::Object(hooks)) = settings.get_mut("hooks") else {
        return;
    };
    for groups in hooks.values_mut() {
        if let Value::Array(groups) = groups {
            for group in groups.iter_mut() {
                if let Some(Value::Array(list)) = group.get_mut("hooks") {
                    list.retain(|hook| !is_ours(hook));
                }
            }
            groups.retain(|g| {
                g.get("hooks")
                    .and_then(Value::as_array)
                    .is_none_or(|l| !l.is_empty())
            });
        }
    }
    hooks.retain(|_, groups| groups.as_array().is_none_or(|g| !g.is_empty()));
    if hooks.is_empty() {
        settings.remove("hooks");
    }
}

fn write_settings(path: &Path, settings: &Map<String, Value>) -> io::Result<()> {
    if read_settings(path).is_ok_and(|current| &current == settings) {
        return Ok(());
    }
    // Write through a symlinked settings.json (dotfiles) instead of replacing the link.
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // One backup of the user's original file, made before Solace first changes it.
    let backup = path.with_extension("json.solace-backup");
    if path.exists() && !backup.exists() {
        fs::copy(path, &backup)?;
    }
    crate::persist::save(path, settings)
}

pub fn install_claude(paths: &ClaudePaths, solace_exe: &Path) -> io::Result<()> {
    if !solace_exe.is_file() {
        return Err(io::Error::other(format!(
            "{} not found",
            solace_exe.display()
        )));
    }
    let mut settings = read_settings(&paths.settings)?;
    strip_ours(&mut settings);
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::other("\"hooks\" in settings.json is not an object"))?;
    let entry = json!({ "hooks": [{ "type": "command", "command": hook_command(solace_exe), "timeout": 5 }] });
    for event in CLAUDE_EVENTS {
        let groups = hooks.entry(*event).or_insert_with(|| json!([]));
        groups
            .as_array_mut()
            .ok_or_else(|| io::Error::other(format!("hooks.{event} is not a list")))?
            .push(entry.clone());
    }
    write_settings(&paths.settings, &settings)?;
    if let Some(dir) = paths.skill.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&paths.skill, SKILL)
}

pub fn uninstall_claude(paths: &ClaudePaths) -> io::Result<()> {
    if paths.settings.exists() {
        let mut settings = read_settings(&paths.settings)?;
        strip_ours(&mut settings);
        write_settings(&paths.settings, &settings)?;
    }
    match fs::remove_file(&paths.skill) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

pub fn claude_installed(paths: &ClaudePaths) -> bool {
    read_settings(&paths.settings).is_ok_and(|settings| {
        settings
            .get("hooks")
            .and_then(|h| h.get("Stop"))
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|l| l.iter().any(is_ours))
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_idempotently_and_keeps_the_users_own_hooks() {
        let home = std::env::temp_dir().join(format!("solace-claude-test-{}", std::process::id()));
        let paths = ClaudePaths::in_home(&home);
        fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        let mine = json!({
            "zeta": 1,
            "model": "opus",
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }] }
        });
        fs::write(&paths.settings, format!("\u{feff}{mine}")).unwrap();

        let exe = &home.join("solace.exe");
        fs::write(exe, "").unwrap();
        install_claude(&paths, exe).unwrap();
        install_claude(&paths, exe).unwrap();
        let text = fs::read_to_string(&paths.settings).unwrap();
        assert!(
            text.find("zeta") < text.find("model"),
            "the user's key order is kept"
        );
        let settings = read_settings(&paths.settings).unwrap();
        assert_eq!(settings["model"], "opus");
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "the user's hook plus exactly one of ours");
        assert!(claude_installed(&paths));
        assert!(paths.skill.exists());
        assert!(paths.settings.with_extension("json.solace-backup").exists());

        uninstall_claude(&paths).unwrap();
        let settings = read_settings(&paths.settings).unwrap();
        assert_eq!(
            settings["hooks"], mine["hooks"],
            "only Solace's hooks are removed"
        );
        assert!(!claude_installed(&paths));
        assert!(!paths.skill.exists());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_settings_it_cannot_read() {
        let home = std::env::temp_dir().join(format!("solace-claude-bad-{}", std::process::id()));
        let paths = ClaudePaths::in_home(&home);
        fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        fs::write(&paths.settings, "{ not json").unwrap();
        let exe = &home.join("solace.exe");
        fs::write(exe, "").unwrap();
        assert!(install_claude(&paths, exe).is_err());
        assert_eq!(fs::read_to_string(&paths.settings).unwrap(), "{ not json");
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn hook_command_runs_in_bash_from_a_verbatim_windows_path() {
        let command = hook_command(Path::new(
            r"\\?\C:\Users\Ana B\AppData\Local\Solace\bin\solace.exe",
        ));
        assert_eq!(
            command,
            "\"C:/Users/Ana B/AppData/Local/Solace/bin/solace.exe\" hook --source solace-hook 2>/dev/null || true"
        );
    }
}
