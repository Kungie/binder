use binder_core::hooks::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const CLI: &str = "/Applications/Binder.app/Contents/MacOS/binder-cli";

fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/claude/settings.json"), &path).unwrap();
    (dir, path)
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn commands(v: &Value, event: &str) -> Vec<String> {
    v["hooks"][event]
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .flat_map(|g| g["hooks"].as_array().unwrap().iter().map(|h| h["command"].as_str().unwrap().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn install_adds_three_hooks_and_keeps_others() {
    let (_d, path) = fixture();
    install(&path, Path::new(CLI)).unwrap();
    let v = read(&path);
    assert_eq!(commands(&v, "Stop"), ["say done".to_string(), format!("{CLI} hook stop")]);
    assert_eq!(commands(&v, "PreCompact"), [format!("{CLI} hook precompact")]);
    assert_eq!(commands(&v, "SessionEnd"), [format!("{CLI} hook session-end")]);
    assert_eq!(commands(&v, "Notification"), ["afplay /System/Library/Sounds/Glass.aiff"]);
    assert_eq!(v["model"], "opus");
    assert_eq!(v["permissions"], json!({"allow": ["Bash(git status)"]}));
    assert_eq!(v["statusLine"]["command"], "echo hi");
    // Keys keep their order.
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["model", "permissions", "hooks", "statusLine"]);
    assert!(installed(&path));
}

#[test]
fn install_twice_is_one_entry() {
    let (_d, path) = fixture();
    install(&path, Path::new(CLI)).unwrap();
    install(&path, Path::new("/elsewhere/binder-cli")).unwrap();
    let v = read(&path);
    assert_eq!(commands(&v, "Stop"), ["say done", "/elsewhere/binder-cli hook stop"]);
    assert_eq!(commands(&v, "PreCompact").len(), 1);
}

#[test]
fn uninstall_removes_only_ours() {
    let (_d, path) = fixture();
    let before = read(&path);
    install(&path, Path::new(CLI)).unwrap();
    uninstall(&path).unwrap();
    assert_eq!(read(&path), before);
    assert!(!installed(&path));
}

#[test]
fn missing_settings_file_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".claude/settings.json");
    assert!(!installed(&path));
    install(&path, Path::new("/Apps/My Tools/binder-cli")).unwrap();
    let v = read(&path);
    assert_eq!(commands(&v, "SessionEnd"), ["'/Apps/My Tools/binder-cli' hook session-end"]);
    assert!(installed(&path));
    uninstall(&path).unwrap();
    assert!(!installed(&path));
}
