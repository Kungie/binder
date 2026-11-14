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
    Config { vault: "/tmp/v".into(), ..Config::default() }.save(&path).unwrap();
    assert_eq!(Config::load(&path).unwrap().vault, Path::new("/tmp/v"));
}

#[test]
fn cloud_remote_defaults_to_srv_cloud() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Config::load(&dir.path().join("config.toml")).unwrap().cloud_remote, "/srv/cloud");
    std::fs::write(dir.path().join("old.toml"), "vault = \"/v\"\n").unwrap();
    assert_eq!(Config::load(&dir.path().join("old.toml")).unwrap().cloud_remote, "/srv/cloud", "a 2.1 file gets the default");
}

#[test]
fn config_defaults_for_new_fields() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("old.toml"), "vault = \"/Volumes/vault\"\nclaude_hooks = true\n").unwrap();
    let c = Config::load(&dir.path().join("old.toml")).unwrap();
    assert_eq!((c.pick_up, c.watch, c.zoom), (true, true, 100));
}
