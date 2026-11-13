//! Binder's three Claude Code hooks in `~/.claude/settings.json`. Only entries that run `binder-cli hook` are
//! ever added or removed; everything else in the file stays as it was, key order included.

use crate::Result;
use crate::remote::quote;
use serde_json::{Map, Value, json};
use std::path::Path;

/// Claude Code's hook event, and the argument `binder-cli hook` takes for it.
pub const EVENTS: [(&str, &str); 3] = [("Stop", "stop"), ("PreCompact", "precompact"), ("SessionEnd", "session-end")];

fn ours(command: &str) -> bool {
    command.contains("binder-cli") && command.contains(" hook ")
}

fn shell_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c)) { s.into_owned() } else { quote(&s) }
}

fn read(path: &Path) -> Result<Map<String, Value>> {
    match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(map),
            _ => Err(crate::Error::Invalid(format!("{} isn't a JSON object, so Binder left it alone.", path.display()))),
        },
        Ok(_) => Ok(Map::new()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e.into()),
    }
}

fn write(path: &Path, settings: &Map<String, Value>) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string_pretty(settings).expect("a JSON map serializes");
    text.push('\n');
    // Write beside it and rename, so a crash never leaves Claude Code a half-written file.
    let tmp = path.with_extension("json.binder-tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Drops our commands from one event's groups, and groups left empty by that.
fn strip(groups: &mut Vec<Value>) {
    for g in groups.iter_mut() {
        if let Some(hooks) = g.get_mut("hooks").and_then(Value::as_array_mut) {
            hooks.retain(|h| !h.get("command").and_then(Value::as_str).is_some_and(ours));
        }
    }
    groups.retain(|g| g.get("hooks").and_then(Value::as_array).is_none_or(|h| !h.is_empty()));
}

pub fn install(settings_path: &Path, cli: &Path) -> Result<()> {
    let mut settings = read(settings_path)?;
    let hooks = settings.entry("hooks").or_insert_with(|| Value::Object(Map::new()));
    if !hooks.is_object() {
        *hooks = Value::Object(Map::new());
    }
    let hooks = hooks.as_object_mut().expect("just made an object");
    for (event, arg) in EVENTS {
        let groups = hooks.entry(event).or_insert_with(|| Value::Array(vec![]));
        if !groups.is_array() {
            *groups = Value::Array(vec![]);
        }
        let groups = groups.as_array_mut().expect("just made an array");
        strip(groups);
        groups.push(json!({ "hooks": [{ "type": "command", "command": format!("{} hook {arg}", shell_path(cli)) }] }));
    }
    write(settings_path, &settings)
}

pub fn uninstall(settings_path: &Path) -> Result<()> {
    if !settings_path.exists() {
        return Ok(());
    }
    let mut settings = read(settings_path)?;
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else { return Ok(()) };
    for (event, _) in EVENTS {
        if let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) {
            strip(groups);
            if groups.is_empty() {
                hooks.remove(event);
            }
        }
    }
    if hooks.is_empty() {
        settings.remove("hooks");
    }
    write(settings_path, &settings)
}

/// Whether all three of Binder's hooks are in the file.
pub fn installed(settings_path: &Path) -> bool {
    let Ok(settings) = read(settings_path) else { return false };
    EVENTS.iter().all(|(event, _)| {
        settings
            .get("hooks")
            .and_then(|h| h.get(*event))
            .and_then(Value::as_array)
            .is_some_and(|groups| {
                groups.iter().any(|g| {
                    g.get("hooks").and_then(Value::as_array).is_some_and(|hs| hs.iter().any(|h| h.get("command").and_then(Value::as_str).is_some_and(ours)))
                })
            })
    })
}
