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
    format!(
        "\"{}\" hook --source {MARKER} 2>/dev/null || true",
        bash_path(solace_exe)
    )
}

fn bash_path(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    // Tauri's paths are verbatim (`\\?\C:\...`), which bash can't run; forward slashes it can.
    path.strip_prefix(r"\\?\")
        .unwrap_or(&path)
        .replace('\\', "/")
}

/// Splits our status line from the user's own, which keeps running behind it.
/// If Solace is gone, `|| cat` still passes the status JSON on to the user's command.
const WRAP: &str = " 2>/dev/null || cat; } | {\n";

/// Claude Code's status line command: ours alone, or ours teeing the status JSON into the user's
/// `original`. The newlines keep a trailing `# comment` in the original from eating the brace.
fn statusline_command(solace_exe: &Path, original: Option<&str>) -> String {
    let exe = bash_path(solace_exe);
    match original {
        None => format!("\"{exe}\" statusline --source {MARKER} 2>/dev/null || true"),
        Some(orig) => {
            format!("{{ \"{exe}\" statusline --tee --source {MARKER}{WRAP}{orig}\n}}")
        }
    }
}

/// `None`: not ours, or a wrapper the user edited beyond recognition (left alone). `Some(None)`:
/// we created it. `Some(Some(orig))`: wrapped around the user's.
fn original_statusline(command: &str) -> Option<Option<String>> {
    if !command.contains(MARKER) {
        return None;
    }
    let Some((_, rest)) = command.split_once(WRAP) else {
        return Some(None);
    };
    let orig = rest
        .strip_suffix("\n}")
        .or_else(|| rest.strip_suffix('}'))?;
    Some(Some(orig.to_string()))
}

/// Puts the user's status line back, or removes the one we created.
fn strip_statusline(settings: &mut Map<String, Value>) {
    let original = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(Value::as_str)
        .and_then(original_statusline);
    match original {
        Some(Some(orig)) => settings["statusLine"]["command"] = Value::String(orig),
        Some(None) => {
            settings.remove("statusLine");
        }
        None => {}
    }
}

/// Adds our status line, wrapping the user's if they have one; a status line without a command
/// (or not an object) is left alone.
fn add_statusline(settings: &mut Map<String, Value>, solace_exe: &Path) {
    match settings.get_mut("statusLine") {
        None => {
            settings.insert(
                "statusLine".into(),
                json!({ "type": "command", "command": statusline_command(solace_exe, None) }),
            );
        }
        Some(Value::Object(sl)) => {
            if let Some(orig) = sl.get("command").and_then(Value::as_str) {
                let wrapped = statusline_command(solace_exe, Some(orig));
                sl.insert("command".into(), Value::String(wrapped));
            }
        }
        Some(_) => {}
    }
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
    strip_statusline(settings);
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
    add_statusline(&mut settings, solace_exe);
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
    fn status_line_is_added_wrapped_and_restored() {
        let home = std::env::temp_dir().join(format!("solace-claude-sl-{}", std::process::id()));
        let paths = ClaudePaths::in_home(&home);
        fs::create_dir_all(paths.settings.parent().unwrap()).unwrap();
        let exe = &home.join("solace.exe");
        let exe2 = &home.join("solace2.exe");
        fs::write(exe, "").unwrap();
        fs::write(exe2, "").unwrap();

        // None of the user's own: added, then removed.
        fs::write(&paths.settings, "{}").unwrap();
        install_claude(&paths, exe).unwrap();
        install_claude(&paths, exe).unwrap();
        let sl = &read_settings(&paths.settings).unwrap()["statusLine"];
        assert!(
            sl["command"]
                .as_str()
                .unwrap()
                .contains("statusline --source solace-hook")
        );
        uninstall_claude(&paths).unwrap();
        assert!(
            read_settings(&paths.settings)
                .unwrap()
                .get("statusLine")
                .is_none()
        );

        // The user's own: wrapped once, restored exactly.
        let mine = json!({ "statusLine": { "type": "command", "command": "a && b # note", "padding": 2 } });
        fs::write(&paths.settings, mine.to_string()).unwrap();
        install_claude(&paths, exe).unwrap();
        install_claude(&paths, exe).unwrap();
        install_claude(&paths, exe2).unwrap();
        let settings = read_settings(&paths.settings).unwrap();
        let command = settings["statusLine"]["command"].as_str().unwrap();
        assert!(
            command.starts_with("{ \"")
                && command.contains("solace2.exe\" statusline --tee --source solace-hook")
        );
        assert!(command.contains("|| cat; } | {\n"));
        // Text added after the closing brace: not ours to restore.
        assert_eq!(original_statusline(&format!("{command} # mine")), None);
        assert_eq!(
            original_statusline(&command.replace("\n}", "}")),
            Some(Some("a && b # note".to_string()))
        );
        assert_eq!(
            original_statusline(command),
            Some(Some("a && b # note".to_string()))
        );
        assert_eq!(settings["statusLine"]["padding"], 2);
        uninstall_claude(&paths).unwrap();
        assert_eq!(
            read_settings(&paths.settings).unwrap()["statusLine"],
            mine["statusLine"]
        );

        // No command to wrap: left alone.
        let bare = json!({ "statusLine": { "type": "static" } });
        fs::write(&paths.settings, bare.to_string()).unwrap();
        install_claude(&paths, exe).unwrap();
        assert_eq!(
            read_settings(&paths.settings).unwrap()["statusLine"],
            bare["statusLine"]
        );

        assert_eq!(original_statusline("echo hi"), None);
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
