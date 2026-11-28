//! Installs Solace's integration into the agent CLIs: hooks or a status line that report each
//! agent's exact state (and context use), and the skill that teaches it the `solace` CLI.
//! Claude Code: hooks and status line in `~/.claude/settings.json`. Codex: `~/.codex/hooks.json`.
//! Kimi Code: `[[hooks]]` in `~/.kimi-code/config.toml`. Antigravity: its status line in
//! `~/.gemini/antigravity-cli/settings.json`. OpenCode: only the skill; its screen is read.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use solace_core::agents::hooks::{CLAUDE_EVENTS, CODEX_EVENTS, KIMI_EVENTS};

const SKILL: &str = include_str!("../../../skills/solace/SKILL.md");
/// Marks hook commands as Solace's, so updates and uninstalls touch nothing else.
const MARKER: &str = "solace-hook";

pub struct ClaudePaths {
    pub settings: PathBuf,
    pub skill: PathBuf,
    /// Codex and several other agents read skills from here.
    pub agents_skill: PathBuf,
}

impl ClaudePaths {
    pub fn in_home(home: &Path) -> Self {
        let claude = home.join(".claude");
        ClaudePaths {
            settings: claude.join("settings.json"),
            skill: claude.join("skills").join("solace").join("SKILL.md"),
            agents_skill: home.join(".agents/skills/solace/SKILL.md"),
        }
    }
}

/// The command Claude Code runs (in Git Bash on Windows) for every listened event. The `|| true`
/// keeps any failure, such as Solace being uninstalled, from blocking Claude.
fn hook_command(solace_exe: &Path) -> String {
    format!(
        "{} hook --source {MARKER} 2>/dev/null || true",
        quote(bash_path(solace_exe))
    )
}

/// The path as one bash word. Double quotes would still expand `$` and backticks in a folder name.
fn quote(path: String) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
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
    let exe = quote(bash_path(solace_exe));
    match original {
        None => format!("{exe} statusline --source {MARKER} 2>/dev/null || true"),
        Some(orig) => {
            format!("{{ {exe} statusline --tee --source {MARKER}{WRAP}{orig}\n}}")
        }
    }
}

/// How Antigravity's status line wraps the user's under cmd.exe, which has no `{ }` groups.
const CMD_WRAP: &str = " --source solace-hook | ";

/// `None`: not ours, or a wrapper the user edited beyond recognition (left alone). `Some(None)`:
/// we created it. `Some(Some(orig))`: wrapped around the user's.
fn original_statusline(command: &str) -> Option<Option<String>> {
    if !command.contains(MARKER) {
        return None;
    }
    if !command.contains(WRAP)
        && let Some((_, orig)) = command.split_once(CMD_WRAP)
    {
        return Some(Some(orig.to_string()));
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

fn check_exe(solace_exe: &Path) -> io::Result<()> {
    if solace_exe.is_file() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{} not found",
            solace_exe.display()
        )))
    }
}

fn write_skill(skill: &Path) -> io::Result<()> {
    if let Some(dir) = skill.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(skill, SKILL)
}

pub fn install_claude(paths: &ClaudePaths, solace_exe: &Path) -> io::Result<()> {
    check_exe(solace_exe)?;
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
    write_skill(&paths.skill)?;
    write_skill(&paths.agents_skill)
}

/// Removes our SKILL.md, and its folder when nothing else is in it.
fn remove_skill(skill: &Path) -> io::Result<()> {
    match fs::remove_file(skill) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
        _ => {}
    }
    if let Some(dir) = skill.parent() {
        let _ = fs::remove_dir(dir); // fails, as wanted, when the user keeps other files there
    }
    Ok(())
}

/// Takes Solace out of a JSON settings file, if the file is there.
fn strip_file(path: &Path) -> io::Result<()> {
    if path.exists() {
        let mut settings = read_settings(path)?;
        strip_ours(&mut settings);
        write_settings(path, &settings)?;
    }
    Ok(())
}

/// Claude's own pieces; the shared `~/.agents` skill is left to [`uninstall`].
fn uninstall_claude_only(paths: &ClaudePaths) -> io::Result<()> {
    strip_file(&paths.settings)?;
    remove_skill(&paths.skill)
}

pub fn uninstall_claude(paths: &ClaudePaths) -> io::Result<()> {
    uninstall_claude_only(paths)?;
    remove_skill(&paths.agents_skill)
}

/// Whether `hooks.<event>` holds one of our commands.
fn has_our_hook(settings: &Map<String, Value>, event: &str) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|g| {
                g.get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(is_ours))
            })
        })
}

pub fn claude_installed(paths: &ClaudePaths) -> bool {
    read_settings(&paths.settings).is_ok_and(|settings| has_our_hook(&settings, "Stop"))
}

/// The CLIs Solace integrates with, by catalog id.
pub const AGENTS: &[&str] = &["claude", "codex", "kimi", "antigravity", "opencode"];

/// The path as cmd.exe runs it, unquoted (Codex and Kimi run hooks with `cmd /c`; quotes inside a
/// command handed to cmd by Node or Go come out escaped and break it). Paths with spaces use
/// their 8.3 short form, which has none.
fn cmd_path(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let path = path.strip_prefix(r"\\?\").unwrap_or(&path).to_string();
    if !path.contains(' ') {
        return path;
    }
    #[cfg(windows)]
    if let Some(short) = short_path(&path) {
        return short;
    }
    format!("\"{path}\"")
}

#[cfg(windows)]
fn short_path(path: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let wide = widestring::U16CString::from_str(path).ok()?;
    let mut buf = vec![0u16; 1024];
    // SAFETY: both buffers are valid for the lengths passed.
    let len = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    let short = String::from_utf16(buf.get(..len as usize)?).ok()?;
    (len > 0 && !short.contains(' ')).then_some(short)
}

/// `solace <args>` for the shell an agent runs commands in: cmd.exe on Windows, sh elsewhere.
fn agent_command(solace_exe: &Path, args: &str) -> String {
    if cfg!(windows) {
        format!("{} {args}", cmd_path(solace_exe))
    } else {
        format!(
            "{} {args} 2>/dev/null || true",
            quote(bash_path(solace_exe))
        )
    }
}

fn codex_hooks(home: &Path) -> PathBuf {
    home.join(".codex").join("hooks.json")
}

fn install_codex(home: &Path, solace_exe: &Path) -> io::Result<()> {
    let path = codex_hooks(home);
    let mut settings = read_settings(&path)?;
    strip_ours(&mut settings);
    let hooks = settings
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::other("\"hooks\" in hooks.json is not an object"))?;
    let args = format!("hook --agent codex --source {MARKER}");
    let handler = json!({
        "type": "command",
        "command": format!("{} {args} 2>/dev/null || true", quote(bash_path(solace_exe))),
        "commandWindows": format!("{} {args}", cmd_path(solace_exe)),
        // Codex caps SessionEnd and Interrupt hooks at 3 s.
        "timeout": 3,
    });
    for event in CODEX_EVENTS {
        hooks
            .entry(*event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| io::Error::other(format!("hooks.{event} is not a list")))?
            .push(json!({ "hooks": [handler.clone()] }));
    }
    write_settings(&path, &settings)
}

const KIMI_BEGIN: &str = "# >>> solace: added by Solace; removed when its Kimi integration is off";
const KIMI_END: &str = "# <<< solace";

fn kimi_config(home: &Path) -> PathBuf {
    home.join(".kimi-code").join("config.toml")
}

/// The config without our block (and the blank line before it).
fn strip_kimi(text: &str) -> String {
    let Some(start) = text.find(KIMI_BEGIN) else {
        return text.to_string();
    };
    let end = text[start..]
        .find(KIMI_END)
        .map_or(text.len(), |i| start + i + KIMI_END.len());
    let rest = text[end..].strip_prefix('\n').unwrap_or(&text[end..]);
    format!("{}{rest}", text[..start].trim_end_matches('\n'))
        .trim_end()
        .to_string()
        + "\n"
}

fn install_kimi(home: &Path, solace_exe: &Path) -> io::Result<()> {
    let path = kimi_config(home);
    let current = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let command = toml::Value::String(agent_command(
        solace_exe,
        &format!("hook --agent kimi --source {MARKER}"),
    ));
    let mut text = strip_kimi(&current);
    text.push_str(&format!("\n{KIMI_BEGIN}\n"));
    for event in KIMI_EVENTS {
        text.push_str(&format!(
            "[[hooks]]\nevent = \"{event}\"\ncommand = {command}\ntimeout = 5\n"
        ));
    }
    text.push_str(KIMI_END);
    text.push('\n');
    // Never leave Kimi a config it can't load: the user's part must already be valid TOML.
    text.parse::<toml::Table>()
        .map_err(|e| io::Error::other(format!("{}: {e}", path.display())))?;
    write_text(&path, &current, &text)
}

/// Writes a config the user also edits by hand, keeping one backup of their original.
fn write_text(path: &Path, current: &str, text: &str) -> io::Result<()> {
    if current == text {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let backup = path.with_extension("toml.solace-backup");
    if path.exists() && !backup.exists() {
        fs::copy(path, &backup)?;
    }
    fs::write(path, text)
}

fn uninstall_kimi(home: &Path) -> io::Result<()> {
    let path = kimi_config(home);
    match fs::read_to_string(&path) {
        Ok(text) if text.contains(KIMI_BEGIN) => write_text(&path, &text, &strip_kimi(&text)),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn agy_settings(home: &Path) -> PathBuf {
    home.join(".gemini")
        .join("antigravity-cli")
        .join("settings.json")
}

fn agy_skill(home: &Path) -> PathBuf {
    home.join(".gemini/config/skills/solace/SKILL.md")
}

/// Antigravity reruns its status line whenever the agent's state changes: the state and context
/// use arrive through it. Ours prints nothing, so Antigravity's own status line stays.
fn install_antigravity(home: &Path, solace_exe: &Path) -> io::Result<()> {
    let path = agy_settings(home);
    let mut settings = read_settings(&path)?;
    strip_statusline(&mut settings);
    let ours = |tee: &str| {
        agent_command(
            solace_exe,
            &format!("statusline{tee} --agent antigravity --source {MARKER}"),
        )
    };
    match settings.get_mut("statusLine") {
        None => {
            settings.insert(
                "statusLine".into(),
                json!({ "type": "command", "command": ours(""), "stack_with_default": true }),
            );
        }
        Some(Value::Object(sl)) => {
            if let Some(orig) = sl.get("command").and_then(Value::as_str) {
                let wrapped = if cfg!(windows) {
                    format!("{}{CMD_WRAP}{orig}", ours(" --tee"))
                } else {
                    statusline_command(solace_exe, Some(orig))
                        .replace("--tee --source", "--tee --agent antigravity --source")
                };
                sl.insert("command".into(), Value::String(wrapped));
            }
        }
        Some(_) => {}
    }
    write_settings(&path, &settings)?;
    write_skill(&agy_skill(home))
}

fn opencode_skill(home: &Path) -> PathBuf {
    home.join(".config/opencode/skills/solace/SKILL.md")
}

/// Installs one agent's integration; `agent` is a catalog id from [`AGENTS`].
pub fn install(agent: &str, home: &Path, solace_exe: &Path) -> io::Result<()> {
    check_exe(solace_exe)?;
    let shared = home.join(".agents/skills/solace/SKILL.md");
    match agent {
        "claude" => install_claude(&ClaudePaths::in_home(home), solace_exe),
        "codex" => install_codex(home, solace_exe).and_then(|()| write_skill(&shared)),
        "kimi" => install_kimi(home, solace_exe).and_then(|()| write_skill(&shared)),
        "antigravity" => install_antigravity(home, solace_exe),
        "opencode" => write_skill(&opencode_skill(home)),
        _ => Err(io::Error::other(format!("no integration for {agent}"))),
    }
}

pub fn uninstall(agent: &str, home: &Path) -> io::Result<()> {
    match agent {
        "claude" => uninstall_claude_only(&ClaudePaths::in_home(home))?,
        "codex" => strip_file(&codex_hooks(home))?,
        "kimi" => uninstall_kimi(home)?,
        "antigravity" => {
            strip_file(&agy_settings(home))?;
            remove_skill(&agy_skill(home))?;
        }
        "opencode" => remove_skill(&opencode_skill(home))?,
        _ => return Err(io::Error::other(format!("no integration for {agent}"))),
    }
    // Claude, Codex and Kimi share this copy; it goes with the last of them.
    if !["claude", "codex", "kimi"]
        .iter()
        .any(|a| installed(a, home))
    {
        remove_skill(&home.join(".agents/skills/solace/SKILL.md"))?;
    }
    Ok(())
}

pub fn installed(agent: &str, home: &Path) -> bool {
    match agent {
        "claude" => claude_installed(&ClaudePaths::in_home(home)),
        "codex" => read_settings(&codex_hooks(home)).is_ok_and(|s| has_our_hook(&s, "Stop")),
        "kimi" => fs::read_to_string(kimi_config(home)).is_ok_and(|t| t.contains(KIMI_BEGIN)),
        "antigravity" => read_settings(&agy_settings(home)).is_ok_and(|s| {
            s.get("statusLine")
                .and_then(|sl| sl.get("command"))
                .and_then(Value::as_str)
                .is_some_and(|c| c.contains(MARKER))
        }),
        "opencode" => opencode_skill(home).is_file(),
        _ => false,
    }
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
        assert!(paths.skill.exists() && paths.agents_skill.exists());
        assert!(paths.settings.with_extension("json.solace-backup").exists());

        uninstall_claude(&paths).unwrap();
        let settings = read_settings(&paths.settings).unwrap();
        assert_eq!(
            settings["hooks"], mine["hooks"],
            "only Solace's hooks are removed"
        );
        assert!(!claude_installed(&paths));
        assert!(!paths.skill.exists() && !paths.agents_skill.exists());
        assert!(
            !paths.agents_skill.parent().unwrap().exists(),
            "an empty folder goes too"
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn uninstall_keeps_other_files_in_the_skill_folder() {
        let home = std::env::temp_dir().join(format!("solace-claude-skill-{}", std::process::id()));
        let paths = ClaudePaths::in_home(&home);
        let exe = &home.join("solace.exe");
        fs::create_dir_all(&home).unwrap();
        fs::write(exe, "").unwrap();
        install_claude(&paths, exe).unwrap();
        let mine = paths.agents_skill.with_file_name("notes.md");
        fs::write(&mine, "mine").unwrap();
        uninstall_claude(&paths).unwrap();
        assert!(mine.exists() && !paths.agents_skill.exists());
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
            command.starts_with("{ '")
                && command.contains("solace2.exe' statusline --tee --source solace-hook")
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
    fn other_agents_install_and_uninstall_cleanly() {
        let home = std::env::temp_dir().join(format!("solace-agents-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(".kimi-code")).unwrap();
        let kimi_mine = "# mine\ndefault_model = \"k2\"\n\n[ui]\ncolor = true\n";
        fs::write(kimi_config(&home), kimi_mine).unwrap();
        fs::create_dir_all(agy_settings(&home).parent().unwrap()).unwrap();
        let agy_mine =
            json!({ "model": "g", "statusLine": { "type": "command", "command": "my-line" } });
        fs::write(agy_settings(&home), agy_mine.to_string()).unwrap();
        let exe = &home.join("solace.exe");
        fs::write(exe, "").unwrap();

        for agent in AGENTS {
            install(agent, &home, exe).unwrap();
            install(agent, &home, exe).unwrap();
            assert!(installed(agent, &home), "{agent}");
        }
        let codex = read_settings(&codex_hooks(&home)).unwrap();
        let stop = codex["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1, "installed once");
        assert!(
            stop[0]["hooks"][0]["commandWindows"]
                .as_str()
                .unwrap()
                .contains("--agent codex")
        );
        let kimi = fs::read_to_string(kimi_config(&home)).unwrap();
        assert_eq!(kimi.matches(KIMI_BEGIN).count(), 1);
        let parsed: toml::Table = kimi.parse().unwrap();
        assert_eq!(parsed["hooks"].as_array().unwrap().len(), KIMI_EVENTS.len());
        assert_eq!(parsed["ui"]["color"].as_bool(), Some(true));
        let agy = read_settings(&agy_settings(&home)).unwrap();
        let line = agy["statusLine"]["command"].as_str().unwrap();
        assert!(line.contains("--agent antigravity") && line.ends_with("my-line"));
        assert!(home.join(".agents/skills/solace/SKILL.md").exists());

        // The shared skill stays while any agent that reads it is still integrated.
        uninstall("codex", &home).unwrap();
        uninstall("kimi", &home).unwrap();
        assert!(home.join(".agents/skills/solace/SKILL.md").exists());
        for agent in AGENTS {
            uninstall(agent, &home).unwrap();
            assert!(!installed(agent, &home), "{agent}");
        }
        assert!(!home.join(".agents/skills/solace/SKILL.md").exists());
        assert_eq!(fs::read_to_string(kimi_config(&home)).unwrap(), kimi_mine);
        assert_eq!(read_settings(&agy_settings(&home)).unwrap(), {
            let Value::Object(m) = agy_mine else {
                unreachable!()
            };
            m
        });
        assert!(
            read_settings(&codex_hooks(&home))
                .unwrap()
                .get("hooks")
                .is_none()
        );
        fs::remove_dir_all(&home).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn cmd_path_has_no_spaces_for_cmd_exe() {
        let dir = std::env::temp_dir().join(format!("solace cmd {}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("solace.exe");
        fs::write(&exe, "").unwrap();
        let path = cmd_path(&exe);
        // The 8.3 name when the volume has them, else quoted (cmd /c strips the outer quotes).
        let ok = if path.starts_with('"') {
            Path::new(path.trim_matches('"')).is_file()
        } else {
            !path.contains(' ') && Path::new(&path).is_file()
        };
        assert!(ok, "{path}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn kimi_refuses_a_config_it_cannot_parse() {
        let home = std::env::temp_dir().join(format!("solace-kimi-bad-{}", std::process::id()));
        fs::create_dir_all(home.join(".kimi-code")).unwrap();
        fs::write(kimi_config(&home), "not = [toml").unwrap();
        let exe = &home.join("solace.exe");
        fs::write(exe, "").unwrap();
        assert!(install("kimi", &home, exe).is_err());
        assert_eq!(
            fs::read_to_string(kimi_config(&home)).unwrap(),
            "not = [toml"
        );
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
            "'C:/Users/Ana B/AppData/Local/Solace/bin/solace.exe' hook --source solace-hook 2>/dev/null || true"
        );
        assert_eq!(quote("/a/it's $HOME".into()), r"'/a/it'\''s $HOME'");
    }

    #[test]
    fn entries_in_the_old_double_quoted_form_are_still_removed() {
        let old = r#"{ "C:/Solace/solace.exe" statusline --tee --source solace-hook 2>/dev/null || cat; } | {
a && b
}"#;
        assert_eq!(original_statusline(old), Some(Some("a && b".to_string())));
        let mut settings = read_settings(Path::new("none")).unwrap();
        settings.insert(
            "hooks".into(),
            json!({ "Stop": [{ "hooks": [{ "type": "command",
                "command": "\"C:/Solace/solace.exe\" hook --source solace-hook 2>/dev/null || true" }] }] }),
        );
        strip_ours(&mut settings);
        assert!(settings.get("hooks").is_none());
    }
}
