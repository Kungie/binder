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

#[test]
fn old_config_turns_print_notify_on() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("old.toml"), "vault = \"/Volumes/vault\"\n").unwrap();
    assert!(Config::load(&dir.path().join("old.toml")).unwrap().print_notify);
}

#[test]
fn old_config_gets_the_default_overview() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("old.toml"), "vault = \"/Volumes/vault\"\n").unwrap();
    let c = Config::load(&dir.path().join("old.toml")).unwrap();
    assert_eq!(c.overview.left, vec!["numbers", "done_today"]);
    assert_eq!(c.overview.right, vec!["server", "cloud", "printer", "open_tasks"]);
}

#[test]
fn overview_sizes_default_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("old.toml"), "[overview]\nleft = [\"numbers\"]\nright = [\"server\"]\n").unwrap();
    let c = Config::load(&dir.path().join("old.toml")).unwrap();
    assert_eq!((c.overview.split, c.overview.heights.len()), (50, 0));
    let mut c = c;
    c.overview.split = 60;
    c.overview.heights.insert("server".into(), 240);
    let path = dir.path().join("new.toml");
    c.save(&path).unwrap();
    let back = Config::load(&path).unwrap();
    assert_eq!((back.overview.split, back.overview.heights.get("server").copied()), (60, Some(240)));
}

#[test]
fn the_vault_share_sits_beside_the_cloud_share() {
    let mut c = binder_core::config::Config::default();
    assert_eq!(c.vault_share_address(), None);
    c.cloud_share = Some("smb://10.0.7.2/cloud".into());
    assert_eq!(c.vault_share_address().as_deref(), Some("smb://10.0.7.2/vault"));
    c.vault_share = Some("smb://10.0.7.3/notes".into());
    assert_eq!(c.vault_share_address().as_deref(), Some("smb://10.0.7.3/notes"));
}

#[test]
fn only_plain_smb_addresses_are_mounted() {
    use binder_core::config::mountable;
    assert!(mountable("smb://10.0.7.2/vault"));
    assert!(mountable("smb://demo-dev@lab.local/vault"));
    assert!(!mountable("afp://10.0.7.2/vault"));
    assert!(!mountable("smb://10.0.7.2/va\"ult"));
    assert!(!mountable("smb://10.0.7.2/vault\\"));
    assert!(!mountable("smb://"));
}
