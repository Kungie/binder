use binder_core::agent::{Access, poll_run, start_run};
use binder_core::model::*;
use binder_core::projects::{PALETTE, color_for};
use binder_core::remote::FakeRemote;
use binder_core::{Db, Error};
use std::sync::Mutex;

fn new(title: &str) -> NewTask {
    NewTask { title: title.into(), ..Default::default() }
}

#[test]
fn migration_v3_keeps_old_data() {
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
             INSERT INTO projects (name, prefix, created) VALUES ('orbit', 'ORB', 1);
             INSERT INTO tasks (key, title, status, created, updated) VALUES ('TSK-1', 'old', 'todo', 1, 1);
             PRAGMA user_version = 1;",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let t = &db.list_tasks(TaskView::All, false).unwrap()[0];
    assert_eq!((t.title.as_str(), t.sort, t.attention, t.due_time.clone()), ("old", 0, false, None));
    let p = &db.list_projects().unwrap()[0];
    assert!(PALETTE.contains(&p.color.as_str()), "{}", p.color);
    assert_eq!((p.github.clone(), p.archived, p.private), (None, false, false));
}

#[test]
fn due_time_set_and_cleared_with_due() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(NewTask { due: Some("2026-11-14".into()), due_time: Some("14:00".into()), ..new("Call") }, 1).unwrap();
    assert_eq!(t.due_time.as_deref(), Some("14:00"));
    let t = db.update_task(t.id, TaskPatch { clear_due_time: true, ..Default::default() }, 2).unwrap();
    assert_eq!((t.due.as_deref(), t.due_time.clone()), (Some("2026-11-14"), None));
    let t = db.update_task(t.id, TaskPatch { due_time: Some("09:30".into()), ..Default::default() }, 3).unwrap();
    assert_eq!(t.due_time.as_deref(), Some("09:30"));
    let t = db.update_task(t.id, TaskPatch { clear_due: true, ..Default::default() }, 4).unwrap();
    assert_eq!((t.due.clone(), t.due_time.clone()), (None, None));
    assert!(matches!(db.update_task(t.id, TaskPatch { due_time: Some("25:00".into()), ..Default::default() }, 5), Err(Error::Invalid(_))));
}

#[test]
fn new_tasks_append_to_their_column() {
    let mut db = Db::in_memory().unwrap();
    let a = db.create_task(new("a"), 1).unwrap();
    let b = db.create_task(new("b"), 2).unwrap();
    let c = db.create_task(NewTask { status: Some(Status::Backlog), ..new("c") }, 3).unwrap();
    assert_eq!((a.sort, b.sort, c.sort), (1, 2, 1));
    let b = db.update_task(b.id, TaskPatch { sort: Some(-5), ..Default::default() }, 4).unwrap();
    assert_eq!(b.sort, -5);
}

#[test]
fn color_for_is_stable_and_in_palette() {
    assert_eq!(PALETTE.len(), 8);
    assert_eq!(color_for("orbit"), color_for("Orbit"));
    assert!(PALETTE.contains(&color_for("homelab")));
    let distinct: std::collections::HashSet<_> = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"].iter().map(|n| color_for(n)).collect();
    assert!(distinct.len() > 2, "the hash spreads names over the palette");
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    assert_eq!(p.color, color_for("orbit"));
}

#[test]
fn bad_color_is_invalid() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    assert!(matches!(db.update_project(p.id, ProjectPatch { color: Some("#123456".into()), ..Default::default() }), Err(Error::Invalid(_))));
    let p = db.update_project(p.id, ProjectPatch { color: Some(PALETTE[2].into()), ..Default::default() }).unwrap();
    assert_eq!(p.color, PALETTE[2]);
}

#[test]
fn ended_run_raises_attention() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(NewTask { assignee: Some(Assignee::Agent), ..new("Prune") }, 1).unwrap();
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok(String::new()), Ok("running\n--binder:tail--\n".into()), Ok("exit:0\n--binder:tail--\n".into())]);
    let run = start_run(&db, &r, t.id, "~", Access::EditsOnly, 5).unwrap();
    poll_run(&db, &r, &run.run, 6).unwrap();
    assert!(!db.lock().unwrap().get_task(t.id).unwrap().attention, "still running");
    poll_run(&db, &r, &run.run, 7).unwrap();
    assert!(db.lock().unwrap().get_task(t.id).unwrap().attention);
    let t = db.lock().unwrap().update_task(t.id, TaskPatch { attention: Some(false), ..Default::default() }, 8).unwrap();
    assert!(!t.attention);
}
