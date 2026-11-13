use binder_core::config::Config;
use binder_core::devices::probe;
use binder_core::model::*;
use binder_core::Db;
use std::net::TcpListener;
use std::time::{Duration, Instant};

#[test]
fn devices_crud_and_order() {
    let mut db = Db::in_memory().unwrap();
    let pi = db.create_device(NewDevice { name: "pi".into(), kind: DeviceKind::Server, address: "10.0.7.20".into(), port: None, note: None }).unwrap();
    db.create_device(NewDevice { name: "Atlas".into(), kind: DeviceKind::Printer, address: "10.0.7.30".into(), port: Some(8883), note: None }).unwrap();
    assert_eq!(pi.port, 22);
    let names: Vec<_> = db.list_devices().unwrap().into_iter().map(|d| d.name).collect();
    assert_eq!(names, ["Atlas", "pi"]);
    let u = db.update_device(pi.id, DevicePatch { note: Some("in the closet".into()), ..Default::default() }).unwrap();
    assert_eq!(u.note, "in the closet");
    db.delete_device(pi.id).unwrap();
    assert_eq!(db.list_devices().unwrap().len(), 1);
}

#[test]
fn probe_sees_open_and_closed_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    assert!(probe("127.0.0.1", port, Duration::from_secs(2)));
    drop(listener);
    let start = Instant::now();
    assert!(!probe("127.0.0.1", port, Duration::from_secs(2)));
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn config_v2_reads_v1_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "vault = \"/Volumes/vault\"\n").unwrap();
    let c = Config::load(&path).unwrap();
    assert_eq!((c.server, c.printers.len(), c.claude_hooks), (None, 0, false));
}

#[test]
fn migration_v2_keeps_v1_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, prefix TEXT NOT NULL UNIQUE, repo TEXT, note TEXT,
                state TEXT NOT NULL DEFAULT 'active', next_number INTEGER NOT NULL DEFAULT 1, created INTEGER NOT NULL);
             CREATE TABLE tasks (id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE, title TEXT NOT NULL, notes TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0, project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
                assignee TEXT NOT NULL DEFAULT 'me', due TEXT, created INTEGER NOT NULL, updated INTEGER NOT NULL);
             CREATE TABLE counters (name TEXT PRIMARY KEY, next INTEGER NOT NULL);
             INSERT INTO tasks (key, title, status, created, updated) VALUES ('TSK-1', 'old', 'todo', 1, 1);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(db.list_tasks(TaskView::All, false).unwrap()[0].title, "old");
    assert!(db.list_devices().unwrap().is_empty());
    assert_eq!(db.list_projects().unwrap().len(), 0);
}
