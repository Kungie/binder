use binder_core::config::Config;
use std::path::Path;

#[test]
fn config_defaults_to_volumes_vault() {
    let dir = tempfile::tempdir().unwrap();
    let c = Config::load(&dir.path().join("config.toml")).unwrap();
    assert_eq!(c.vault, Path::new("/Volumes/vault"));
}

#[test]
fn config_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep/config.toml");
    Config { vault: "/tmp/v".into() }.save(&path).unwrap();
    assert_eq!(Config::load(&path).unwrap().vault, Path::new("/tmp/v"));
}
