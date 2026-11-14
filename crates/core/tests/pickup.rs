use binder_core::agent::{Access, start_run};
use binder_core::model::*;
use binder_core::pickup::{next_task, round};
use binder_core::remote::FakeRemote;
use binder_core::Db;
use std::sync::Mutex;

/// A project with an agent folder, and a way to add agent tasks to it.
fn db() -> (Db, i64) {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    db.update_project(p.id, ProjectPatch { agent_dir: Some("~/code/orbit".into()), ..Default::default() }).unwrap();
    (db, p.id)
}

fn agent_task(db: &mut Db, project: Option<i64>, title: &str, priority: Priority, now: i64) -> Task {
    db.create_task(NewTask { title: title.into(), project_id: project, assignee: Some(Assignee::Agent), priority: Some(priority), ..Default::default() }, now).unwrap()
}

#[test]
fn picks_the_highest_priority_then_the_oldest() {
    let (mut db, p) = db();
    agent_task(&mut db, Some(p), "low", Priority::Low, 1);
    let old_high = agent_task(&mut db, Some(p), "old high", Priority::High, 2);
    agent_task(&mut db, Some(p), "new high", Priority::High, 3);
    let (t, dir) = next_task(&db).unwrap().unwrap();
    assert_eq!((t.id, dir.as_str()), (old_high.id, "~/code/orbit"));
}

#[test]
fn no_priority_comes_last() {
    let (mut db, p) = db();
    agent_task(&mut db, Some(p), "none", Priority::None, 1);
    let low = agent_task(&mut db, Some(p), "low", Priority::Low, 2);
    assert_eq!(next_task(&db).unwrap().unwrap().0.id, low.id);
}

#[test]
fn night_tasks_are_never_picked_up() {
    let (mut db, p) = db();
    let t = agent_task(&mut db, Some(p), "tonight", Priority::Urgent, 1);
    db.update_task(t.id, TaskPatch { night: Some(true), ..Default::default() }, 2).unwrap();
    assert!(next_task(&db).unwrap().is_none());
}

#[test]
fn only_todo_tasks_assigned_to_the_agent() {
    let (mut db, p) = db();
    let mine = db.create_task(NewTask { title: "mine".into(), project_id: Some(p), ..Default::default() }, 1).unwrap();
    let backlog = agent_task(&mut db, Some(p), "later", Priority::High, 2);
    db.update_task(backlog.id, TaskPatch { status: Some(Status::Backlog), ..Default::default() }, 3).unwrap();
    assert!(next_task(&db).unwrap().is_none());
    assert_eq!(mine.assignee, Assignee::Me);
}

#[test]
fn needs_a_project_with_an_agent_folder() {
    let (mut db, _) = db();
    agent_task(&mut db, None, "no project", Priority::High, 1);
    let bare = db.create_project(NewProject { name: "bare".into(), ..Default::default() }, 1).unwrap();
    agent_task(&mut db, Some(bare.id), "no folder", Priority::High, 2);
    assert!(next_task(&db).unwrap().is_none());
}

#[test]
fn a_task_that_ever_ran_is_not_picked_up() {
    let (mut db, p) = db();
    let t = agent_task(&mut db, Some(p), "failed once", Priority::High, 1);
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok(String::new()), Ok("exit:1\n--binder:tail--\nNo.\n".into())]);
    let run = start_run(&db, &r, t.id, "~/code/orbit", Access::EditsOnly, 10).unwrap();
    binder_core::agent::poll_run(&db, &r, &run.run, 20).unwrap();
    let db = db.into_inner().unwrap();
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
    assert!(next_task(&db).unwrap().is_none());
}

#[test]
fn a_round_starts_the_next_task_with_edits_only() {
    let (mut db, p) = db();
    let t = agent_task(&mut db, Some(p), "do it", Priority::High, 1);
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok(String::new())]);
    let done = round(&db, &r, 100, true).unwrap();
    let run = done.started.unwrap();
    assert_eq!((run.task_id, run.access, run.dir.as_str()), (t.id, Access::EditsOnly, "~/code/orbit"));
    assert_eq!(db.lock().unwrap().get_task(t.id).unwrap().status, Status::InProgress);
}

#[test]
fn nothing_starts_while_a_run_is_live() {
    let (mut db, p) = db();
    let busy = agent_task(&mut db, Some(p), "busy", Priority::High, 1);
    agent_task(&mut db, Some(p), "waiting", Priority::High, 2);
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok(String::new()), Ok("running\n--binder:tail--\n".into())]);
    start_run(&db, &r, busy.id, "~/code/orbit", Access::Full, 10).unwrap();
    let done = round(&db, &r, 100, true).unwrap();
    assert_eq!((done.polled, done.started.is_none()), (1, true));
    assert_eq!(r.seen.borrow().len(), 2, "one start, one poll, no second start");
}

#[test]
fn a_finished_run_frees_the_slot_in_the_same_round() {
    let (mut db, p) = db();
    let first = agent_task(&mut db, Some(p), "first", Priority::High, 1);
    let second = agent_task(&mut db, Some(p), "second", Priority::Low, 2);
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok(String::new()), Ok("exit:0\n--binder:tail--\n".into()), Ok(String::new())]);
    start_run(&db, &r, first.id, "~/code/orbit", Access::EditsOnly, 10).unwrap();
    let done = round(&db, &r, 100, true).unwrap();
    assert_eq!(done.started.unwrap().task_id, second.id);
}

#[test]
fn a_round_with_pick_up_off_only_polls() {
    let (mut db, p) = db();
    agent_task(&mut db, Some(p), "waiting", Priority::High, 1);
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![]);
    let done = round(&db, &r, 100, false).unwrap();
    assert_eq!((done.polled, done.started.is_none()), (0, true));
    assert!(r.seen.borrow().is_empty());
}
