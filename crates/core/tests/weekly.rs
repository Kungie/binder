use binder_core::agent::{Access, poll_run, start_run};
use binder_core::events::EventKind;
use binder_core::model::*;
use binder_core::remote::FakeRemote;
use binder_core::{Db, session, weekly};
use chrono::{FixedOffset, NaiveDate, TimeZone};
use std::sync::Mutex;

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}
fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}
/// Unix seconds at a UTC date and hour.
fn at(y: i32, m: u32, day: u32, h: u32) -> i64 {
    utc().with_ymd_and_hms(y, m, day, h, 0, 0).unwrap().timestamp()
}

#[test]
fn due_gives_ended_weeks_up_to_four() {
    // Sunday 15 November 2026 is in week 46; week 45 is the last that ended.
    assert_eq!(weekly::due(None, d(2026, 11, 15)), vec![(2026, 45)]);
    assert_eq!(weekly::due(None, d(2026, 11, 16)), vec![(2026, 46)]);
    assert_eq!(weekly::due(Some((2026, 46)), d(2026, 11, 18)), vec![]);
    assert_eq!(weekly::due(Some((2026, 40)), d(2026, 11, 16)), vec![(2026, 43), (2026, 44), (2026, 45), (2026, 46)]);
    // 2026 has 53 ISO weeks; week 53 ends on 3 January 2027.
    assert_eq!(weekly::due(Some((2026, 52)), d(2027, 1, 5)), vec![(2026, 53)]);
    assert_eq!(weekly::due(Some((2026, 53)), d(2027, 1, 12)), vec![(2027, 1)]);
}

fn week_46_db() -> Mutex<Db> {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    db.update_project(p.id, ProjectPatch { agent_dir: Some("~/orbit".into()), ..Default::default() }).unwrap();
    session::remember(&mut db, p.id, "2026-11-10", Some("14:02"), "Shipped the release notes").unwrap();
    session::remember(&mut db, p.id, "2026-11-17", Some("09:00"), "Next week's line").unwrap();
    let t = db.create_task(NewTask { title: "Write release notes for 0.4".into(), project_id: Some(p.id), ..Default::default() }, 1).unwrap();
    db.update_task(t.id, TaskPatch { status: Some(Status::Done), ..Default::default() }, at(2026, 11, 11, 10)).unwrap();
    db.log_event(at(2026, 11, 12, 2), EventKind::ContainerStopped, "web", "").unwrap();
    db.log_event(at(2026, 11, 12, 3), EventKind::ContainerBack, "web", "").unwrap();
    db.log_event(at(2026, 11, 13, 9), EventKind::PrintDone, "Benchy", "").unwrap();
    let db = Mutex::new(db);
    let run = |title: &str, code: i32, ended: i64| {
        let t = db.lock().unwrap().create_task(NewTask { title: title.into(), project_id: Some(p.id), assignee: Some(Assignee::Agent), ..Default::default() }, 1).unwrap();
        let r = FakeRemote::new(vec![Ok(String::new()), Ok(format!("exit:{code}\n--binder:tail--\n"))]);
        let run = start_run(&db, &r, t.id, "~/orbit", Access::EditsOnly, ended - 60).unwrap();
        poll_run(&db, &r, &run.run, ended).unwrap();
    };
    run("Prune", 0, at(2026, 11, 14, 3));
    run("Break", 1, at(2026, 11, 14, 4));
    db
}

#[test]
fn render_has_each_section() {
    let db = week_46_db();
    let db = db.lock().unwrap();
    let projects = db.list_projects().unwrap();
    let note = weekly::render(&db, &projects, 2026, 46, utc()).unwrap();
    assert!(note.starts_with("# Week 46, 2026\n\n9 to 15 November\n"), "{note}");
    assert!(note.contains("## Done\n### orbit\n- 14:02 · Shipped the release notes\n"), "{note}");
    assert!(!note.contains("Next week's line"));
    assert!(note.contains("## Closed\n- ORB-1 Write release notes for 0.4 (Done)\n"), "{note}");
    assert!(note.contains("## Agent runs\n2 runs, 1 failed: ORB-2, ORB-3 (failed)\n"), "{note}");
    assert!(note.contains("## Server and printer\n1 container alert, 1 print\n"), "{note}");
}

#[test]
fn an_empty_week_says_so() {
    let db = Db::in_memory().unwrap();
    let note = weekly::render(&db, &[], 2026, 40, utc()).unwrap();
    assert_eq!(note.matches("Nothing this week.").count(), 4, "{note}");
    assert!(note.contains("28 September to 4 October"), "{note}");
}

#[test]
fn old_tasks_count_by_updated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    {
        let mut db = Db::open(&path).unwrap();
        let t = db.create_task(NewTask { title: "Old one".into(), ..Default::default() }, 1).unwrap();
        db.update_task(t.id, TaskPatch { status: Some(Status::Canceled), ..Default::default() }, at(2026, 11, 12, 8)).unwrap();
    }
    rusqlite::Connection::open(&path).unwrap().execute_batch("UPDATE tasks SET closed = NULL").unwrap();
    let db = Db::open(&path).unwrap();
    let note = weekly::render(&db, &[], 2026, 46, utc()).unwrap();
    assert!(note.contains("- TSK-1 Old one (Canceled)"), "{note}");
}

#[test]
fn an_existing_note_is_kept() {
    let vault = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(vault.path().join("Weekly")).unwrap();
    std::fs::write(vault.path().join("Weekly/2026-W46.md"), "mine").unwrap();
    let db = week_46_db();
    let mut db = db.lock().unwrap();
    assert_eq!(weekly::write_due(&mut db, vault.path(), d(2026, 11, 16), utc()).unwrap(), 0);
    assert_eq!(std::fs::read_to_string(vault.path().join("Weekly/2026-W46.md")).unwrap(), "mine");
    // It still counts as done for that week.
    assert_eq!(weekly::write_due(&mut db, vault.path(), d(2026, 11, 17), utc()).unwrap(), 0);
}

#[test]
fn without_the_vault_it_waits() {
    let root = tempfile::tempdir().unwrap();
    let vault = root.path().join("vault");
    let db = week_46_db();
    let mut db = db.lock().unwrap();
    assert_eq!(weekly::write_due(&mut db, &vault, d(2026, 11, 16), utc()).unwrap(), 0);
    std::fs::create_dir_all(&vault).unwrap();
    assert_eq!(weekly::write_due(&mut db, &vault, d(2026, 11, 18), utc()).unwrap(), 1);
    let note = std::fs::read_to_string(vault.join("Weekly/2026-W46.md")).unwrap();
    assert!(note.contains("Shipped the release notes"));
}
