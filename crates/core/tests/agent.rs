use binder_core::Error;
use binder_core::agent::*;
use binder_core::model::*;
use binder_core::remote::{FakeRemote, quote};
use binder_core::Db;

fn setup() -> (Db, Task) {
    let mut db = Db::in_memory().unwrap();
    let t = db
        .create_task(NewTask { title: "Prune old images".into(), notes: Some("Keep the last two.".into()), assignee: Some(Assignee::Agent), ..Default::default() }, 1)
        .unwrap();
    (db, t)
}

fn started(replies: Vec<binder_core::Result<String>>) -> (Db, Task, AgentRun, FakeRemote) {
    let (mut db, t) = setup();
    let mut all = vec![Ok(String::new())];
    all.extend(replies);
    let r = FakeRemote::new(all);
    let run = start_run(&mut db, &r, t.id, "~/code", Access::EditsOnly, 1_700_000_000).unwrap();
    (db, t, run, r)
}

#[test]
fn start_sends_quoted_dir_and_prompt_on_stdin() {
    let (mut db, t) = setup();
    let r = FakeRemote::new(vec![Ok(String::new())]);
    let dir = r#"my "proj" $(x)"#;
    let run = start_run(&mut db, &r, t.id, dir, Access::EditsOnly, 1_700_000_000).unwrap();
    assert_eq!(run.run, "tsk-1-1700000000");
    let seen = r.seen.borrow();
    let (script, stdin) = &seen[0];
    assert!(script.contains(&format!("cd {}", quote(dir))), "{script}");
    assert!(script.contains("--permission-mode acceptEdits"));
    assert!(!script.contains("Prune old images"));
    assert_eq!(stdin.as_deref(), Some("Prune old images\n\nKeep the last two."));
}

#[test]
fn start_marks_task_in_progress() {
    let (db, t, run, _) = started(vec![]);
    assert_eq!(db.get_task(t.id).unwrap().status, Status::InProgress);
    assert_eq!(run.state(), RunState::Running);
    assert_eq!(list_runs(&db, t.id).unwrap(), vec![run]);
}

#[test]
fn full_access_uses_skip_permissions() {
    let (mut db, t) = setup();
    let r = FakeRemote::new(vec![Ok(String::new())]);
    start_run(&mut db, &r, t.id, "~", Access::Full, 5).unwrap();
    let script = &r.seen.borrow()[0].0;
    assert!(script.contains("--dangerously-skip-permissions"));
    assert!(!script.contains("acceptEdits"));
    assert!(script.contains("cd \"$HOME\""), "{script}");
}

#[test]
fn poll_exit_zero_marks_done() {
    let (mut db, t, run, r) = started(vec![Ok("exit:0\n--binder:tail--\nPruned 4 images.\n".into())]);
    let p = poll_run(&mut db, &r, &run.run, 1_700_000_100).unwrap();
    assert_eq!(p.state(), RunState::Exited(0));
    assert_eq!((p.ended, p.tail.as_str()), (Some(1_700_000_100), "Pruned 4 images."));
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Done);
}

#[test]
fn poll_failure_returns_to_todo_with_tail() {
    let (mut db, t, run, r) = started(vec![Ok("exit:1\n--binder:tail--\nError: permission denied\n".into())]);
    let p = poll_run(&mut db, &r, &run.run, 9).unwrap();
    assert_eq!(p.state(), RunState::Exited(1));
    assert!(p.tail.contains("permission denied"));
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
}

#[test]
fn poll_while_running_keeps_it_running() {
    let (mut db, t, run, r) = started(vec![Ok("running\n--binder:tail--\nReading files\n".into())]);
    let p = poll_run(&mut db, &r, &run.run, 9).unwrap();
    assert_eq!((p.state(), p.tail.as_str()), (RunState::Running, "Reading files"));
    assert_eq!(db.get_task(t.id).unwrap().status, Status::InProgress);
}

#[test]
fn dead_pid_without_exit_is_lost() {
    let (mut db, t, run, r) = started(vec![Ok("lost\n--binder:tail--\n".into())]);
    let p = poll_run(&mut db, &r, &run.run, 9).unwrap();
    assert_eq!(p.state(), RunState::Lost);
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
}

#[test]
fn stop_kills_group() {
    let (mut db, t, run, r) = started(vec![Ok(String::new())]);
    let p = stop_run(&mut db, &r, &run.run, 9).unwrap();
    assert!(r.seen.borrow()[1].0.contains("kill -- -"));
    assert_eq!(p.state(), RunState::Exited(143));
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
    // A finished run isn't polled again.
    assert_eq!(poll_run(&mut db, &r, &run.run, 10).unwrap(), p);
    assert_eq!(r.seen.borrow().len(), 2);
}

#[test]
fn missing_claude_is_clear_error() {
    let (mut db, t) = setup();
    let r = FakeRemote::new(vec![Err(Error::Remote("Claude Code isn't installed on this server.".into()))]);
    let e = start_run(&mut db, &r, t.id, "~", Access::EditsOnly, 5).unwrap_err();
    assert_eq!(e.to_string(), "Claude Code isn't installed on this server.");
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
    assert!(list_runs(&db, t.id).unwrap().is_empty());
}
