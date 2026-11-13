use binder_core::model::*;
use binder_core::{Db, Error};

fn new(title: &str) -> NewTask {
    NewTask { title: title.into(), ..Default::default() }
}

fn keys(tasks: &[Task]) -> Vec<String> {
    tasks.iter().map(|t| t.key.clone()).collect()
}

#[test]
fn create_without_project_gets_tsk_keys() {
    let mut db = Db::in_memory().unwrap();
    assert_eq!(db.create_task(new("a"), 1).unwrap().key, "TSK-1");
    assert_eq!(db.create_task(new("b"), 2).unwrap().key, "TSK-2");
}

#[test]
fn create_applies_defaults() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(new("Write tests"), 100).unwrap();
    assert_eq!(t.title, "Write tests");
    assert_eq!(t.status, Status::Todo);
    assert_eq!(t.priority, Priority::None);
    assert_eq!(t.assignee, Assignee::Me);
    assert_eq!(t.notes, "");
    assert_eq!(t.project_id, None);
    assert_eq!(t.due, None);
    assert_eq!((t.created, t.updated), (100, 100));
}

#[test]
fn empty_title_is_invalid() {
    let mut db = Db::in_memory().unwrap();
    assert!(matches!(db.create_task(new("  "), 1), Err(Error::Invalid(_))));
}

#[test]
fn list_orders_by_status_then_priority_then_recency() {
    let mut db = Db::in_memory().unwrap();
    let mk = |db: &mut Db, title: &str, status, priority, now| {
        db.create_task(NewTask { title: title.into(), status: Some(status), priority: Some(priority), ..Default::default() }, now).unwrap()
    };
    mk(&mut db, "backlog", Status::Backlog, Priority::Urgent, 1); // TSK-1
    mk(&mut db, "todo low old", Status::Todo, Priority::Low, 2); // TSK-2
    mk(&mut db, "todo none", Status::Todo, Priority::None, 3); // TSK-3
    mk(&mut db, "todo low new", Status::Todo, Priority::Low, 4); // TSK-4
    mk(&mut db, "doing", Status::InProgress, Priority::None, 5); // TSK-5
    mk(&mut db, "todo urgent", Status::Todo, Priority::Urgent, 6); // TSK-6
    let all = db.list_tasks(TaskView::All, false).unwrap();
    assert_eq!(keys(&all), ["TSK-5", "TSK-6", "TSK-4", "TSK-2", "TSK-3", "TSK-1"]);
}

#[test]
fn list_hides_closed_unless_asked() {
    let mut db = Db::in_memory().unwrap();
    db.create_task(NewTask { title: "done".into(), status: Some(Status::Done), ..Default::default() }, 1).unwrap();
    db.create_task(NewTask { title: "gone".into(), status: Some(Status::Canceled), ..Default::default() }, 2).unwrap();
    db.create_task(new("open"), 3).unwrap();
    assert_eq!(keys(&db.list_tasks(TaskView::All, false).unwrap()), ["TSK-3"]);
    assert_eq!(keys(&db.list_tasks(TaskView::All, true).unwrap()), ["TSK-3", "TSK-1", "TSK-2"]);
}

#[test]
fn views_filter_mine_agent_and_project() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "dawn".into(), ..Default::default() }, 1).unwrap();
    db.create_task(new("mine"), 1).unwrap();
    db.create_task(NewTask { title: "agent".into(), assignee: Some(Assignee::Agent), ..Default::default() }, 2).unwrap();
    db.create_task(NewTask { title: "in dawn".into(), project_id: Some(p.id), ..Default::default() }, 3).unwrap();
    let titles = |v| db.list_tasks(v, false).unwrap().into_iter().map(|t| t.title).collect::<Vec<_>>();
    assert_eq!(titles(TaskView::Mine), ["in dawn", "mine"]);
    assert_eq!(titles(TaskView::Agent), ["agent"]);
    assert_eq!(titles(TaskView::Project(p.id)), ["in dawn"]);
}

#[test]
fn update_changes_fields_and_bumps_updated() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(NewTask { title: "a".into(), due: Some("2026-11-20".into()), ..Default::default() }, 1).unwrap();
    let u = db
        .update_task(t.id, TaskPatch { title: Some("b".into()), status: Some(Status::InProgress), notes: Some("n".into()), ..Default::default() }, 9)
        .unwrap();
    assert_eq!((u.title.as_str(), u.status, u.notes.as_str(), u.updated, u.created), ("b", Status::InProgress, "n", 9, 1));
    assert_eq!(u.due.as_deref(), Some("2026-11-20"));
    let cleared = db.update_task(t.id, TaskPatch { clear_due: true, ..Default::default() }, 10).unwrap();
    assert_eq!(cleared.due, None);
    assert_eq!(db.get_task(t.id).unwrap().due, None);
}

#[test]
fn update_missing_task_is_not_found() {
    let mut db = Db::in_memory().unwrap();
    assert!(matches!(db.update_task(99, TaskPatch::default(), 1), Err(Error::NotFound("task", 99))));
}

#[test]
fn delete_removes_task() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(new("a"), 1).unwrap();
    db.delete_task(t.id).unwrap();
    assert!(matches!(db.get_task(t.id), Err(Error::NotFound("task", _))));
}

#[test]
fn reopen_file_keeps_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    {
        let mut db = Db::open(&path).unwrap();
        db.create_task(new("stays"), 1).unwrap();
    }
    let db = Db::open(&path).unwrap();
    assert_eq!(db.list_tasks(TaskView::All, false).unwrap()[0].title, "stays");
}
