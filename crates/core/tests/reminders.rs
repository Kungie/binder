use binder_core::Db;
use binder_core::model::*;
use chrono::NaiveDateTime;

fn at(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
}

fn task(db: &mut Db, title: &str, due: Option<&str>, time: Option<&str>) -> Task {
    db.create_task(NewTask { title: title.into(), due: due.map(Into::into), due_time: time.map(Into::into), ..Default::default() }, 1).unwrap()
}

#[test]
fn a_task_is_reminded_once_when_its_time_comes() {
    let mut db = Db::in_memory().unwrap();
    task(&mut db, "Call the bank", Some("2026-11-17"), Some("09:30"));
    task(&mut db, "No time", Some("2026-11-17"), None);
    task(&mut db, "Later", Some("2026-11-17"), Some("18:00"));
    assert!(db.due_reminders(at("2026-11-17 09:29")).unwrap().is_empty());
    let due = db.due_reminders(at("2026-11-17 09:30")).unwrap();
    assert_eq!(due.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), vec!["Call the bank"]);
    assert!(db.due_reminders(at("2026-11-17 09:31")).unwrap().is_empty());
}

#[test]
fn moving_the_time_reminds_again_and_closed_tasks_stay_quiet() {
    let mut db = Db::in_memory().unwrap();
    let t = task(&mut db, "Call the bank", Some("2026-11-17"), Some("09:30"));
    assert_eq!(db.due_reminders(at("2026-11-17 09:30")).unwrap().len(), 1);
    db.update_task(t.id, TaskPatch { due_time: Some("10:00".into()), ..Default::default() }, 2).unwrap();
    assert_eq!(db.due_reminders(at("2026-11-17 10:00")).unwrap().len(), 1);
    let d = task(&mut db, "Done already", Some("2026-11-17"), Some("11:00"));
    db.update_task(d.id, TaskPatch { status: Some(Status::Done), ..Default::default() }, 3).unwrap();
    assert!(db.due_reminders(at("2026-11-17 11:00")).unwrap().is_empty());
}

#[test]
fn a_reminder_older_than_a_day_is_dropped_quietly() {
    let mut db = Db::in_memory().unwrap();
    task(&mut db, "Long gone", Some("2026-11-10"), Some("09:00"));
    assert!(db.due_reminders(at("2026-11-17 09:00")).unwrap().is_empty());
    // It isn't kept for later either.
    assert!(db.due_reminders(at("2026-11-17 09:01")).unwrap().is_empty());
}
