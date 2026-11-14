use binder_core::agent::{AgentRun, poll_run};
use binder_core::model::*;
use binder_core::night::{self, MARK, SCRIPT};
use binder_core::remote::{FakeRemote, quote};
use binder_core::Db;
use std::sync::Mutex;

const LINE: &str = "0 3 * * * \"$HOME\"/.binder-agent/night.sh # binder-night";

fn night_task(dir: &str) -> (Mutex<Db>, Task) {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    db.update_project(p.id, ProjectPatch { agent_dir: Some(dir.into()), ..Default::default() }).unwrap();
    let t = db.create_task(NewTask { title: "Rotate the logs".into(), notes: Some("Keep a week.".into()), project_id: Some(p.id), ..Default::default() }, 1).unwrap();
    let t = db.update_task(t.id, TaskPatch { night: Some(true), ..Default::default() }, 2).unwrap();
    (Mutex::new(db), t)
}

fn queued(db: &Mutex<Db>) -> Vec<AgentRun> {
    let tasks = db.lock().unwrap().list_tasks(TaskView::All, true).unwrap();
    tasks.iter().flat_map(|t| binder_core::agent::list_runs(&db.lock().unwrap(), t.id).unwrap()).filter(|r| r.queued).collect()
}

#[test]
fn cron_install_keeps_other_lines() {
    let r = FakeRemote::new(vec![Ok("--binder:crontab--\nMAILTO=me\n5 * * * * backup\n".into()), Ok(String::new())]);
    night::cron_install(&r).unwrap();
    let seen = r.seen.borrow();
    assert_eq!(seen[0].1.as_deref(), Some(SCRIPT));
    assert_eq!(seen[1].1.as_deref(), Some(format!("MAILTO=me\n5 * * * * backup\n{LINE}\n").as_str()));
}

#[test]
fn cron_install_on_an_empty_crontab() {
    let r = FakeRemote::new(vec![Ok("motd\n--binder:crontab--\n".into()), Ok(String::new())]);
    night::cron_install(&r).unwrap();
    assert_eq!(r.seen.borrow()[1].1.as_deref(), Some(format!("{LINE}\n").as_str()));
}

#[test]
fn cron_install_twice_adds_nothing() {
    let r = FakeRemote::new(vec![Ok(format!("--binder:crontab--\n{LINE}\n"))]);
    night::cron_install(&r).unwrap();
    assert_eq!(r.seen.borrow().len(), 1);
}

#[test]
fn cron_remove_keeps_other_lines() {
    let r = FakeRemote::new(vec![Ok(format!("--binder:crontab--\nMAILTO=me\n{LINE}\n5 * * * * backup\n")), Ok(String::new())]);
    night::cron_remove(&r).unwrap();
    assert_eq!(r.seen.borrow()[1].1.as_deref(), Some("MAILTO=me\n5 * * * * backup\n"));
}

#[test]
fn cron_state_reads_the_mark() {
    assert!(night::cron_state(&FakeRemote::new(vec![Ok(format!("--binder:crontab--\n{LINE}\n"))])).unwrap());
    assert!(!night::cron_state(&FakeRemote::new(vec![Ok("--binder:crontab--\n5 * * * * backup\n".into())])).unwrap());
    assert!(MARK.starts_with('#'));
}

#[test]
fn sync_queues_a_night_task_once() {
    let (db, t) = night_task("~/code/orbit");
    let r = FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]);
    night::sync(&db, &r, 100).unwrap();
    let q = queued(&db);
    assert_eq!(q.len(), 1);
    assert_eq!(r.seen.borrow()[1].1.as_deref(), Some("Rotate the logs\n\nKeep a week."));
    assert_eq!(db.lock().unwrap().get_task(t.id).unwrap().status, Status::Todo);
    let r2 = FakeRemote::new(vec![Ok(format!("--binder:jobs--\n{}.job\n", q[0].run))]);
    night::sync(&db, &r2, 200).unwrap();
    assert_eq!((queued(&db).len(), r2.seen.borrow().len()), (1, 1));
}

#[test]
fn the_job_quotes_the_folder() {
    let (db, _) = night_task("~/my code/it's");
    let r = FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]);
    night::sync(&db, &r, 100).unwrap();
    let script = r.seen.borrow()[1].0.clone();
    assert!(script.contains(&quote("dir=\"$HOME\"/'my code/it'\\''s'")), "{script}");
}

#[test]
fn sync_drops_a_job_when_night_is_cleared() {
    let (db, t) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    db.lock().unwrap().update_task(t.id, TaskPatch { night: Some(false), ..Default::default() }, 150).unwrap();
    let r = FakeRemote::new(vec![Ok(format!("--binder:jobs--\n{run}.job\n")), Ok(String::new())]);
    night::sync(&db, &r, 200).unwrap();
    assert!(queued(&db).is_empty());
    assert!(r.seen.borrow()[1].0.contains(&format!("{run}.job")));
}

#[test]
fn sync_removes_jobs_binder_no_longer_knows() {
    let mut db = Db::in_memory().unwrap();
    db.create_task(NewTask { title: "x".into(), ..Default::default() }, 1).unwrap();
    let db = Mutex::new(db);
    let r = FakeRemote::new(vec![Ok("--binder:jobs--\ntsk-9-1.job\n".into()), Ok(String::new())]);
    night::sync(&db, &r, 100).unwrap();
    assert!(r.seen.borrow()[1].0.contains("tsk-9-1.job"));
}

#[test]
fn a_queued_run_stays_queued_until_it_starts() {
    let (db, _) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    let p = poll_run(&db, &FakeRemote::new(vec![Ok("--binder:state--\nqueued\n".into())]), &run, 200).unwrap();
    assert_eq!((p.ended, p.queued), (None, true));
}

#[test]
fn a_started_night_run_is_read_like_any_other() {
    let (db, t) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    let r = FakeRemote::new(vec![Ok("--binder:state--\nstarted\n".into()), Ok("--binder:state--\nrunning\n--binder:tail--\nWorking.\n".into())]);
    let p = poll_run(&db, &r, &run, 10_800).unwrap();
    assert_eq!((p.queued, p.started, p.ended), (false, 10_800, None));
    assert_eq!(db.lock().unwrap().get_task(t.id).unwrap().status, Status::InProgress);
}

#[test]
fn clear_empties_the_queue() {
    let (db, _) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    let r = FakeRemote::new(vec![Ok(format!("--binder:jobs--\n{run}.job\n"))]);
    night::clear(&db, &r).unwrap();
    assert!(queued(&db).is_empty());
    assert!(r.seen.borrow()[0].0.contains("rm -f -- *.job"));
}

#[test]
fn night_script_runs_only_from_three_to_seven() {
    assert!(SCRIPT.contains("03|04|05|06"));
    assert!(SCRIPT.contains("*.job"));
    assert!(SCRIPT.starts_with("#!/bin/sh"));
}

#[test]
fn night_script_takes_a_lock_the_system_drops() {
    assert!(SCRIPT.contains("flock -n 9"), "{SCRIPT}");
    assert!(!SCRIPT.contains(".running"));
}

#[test]
fn night_script_starts_each_job_in_its_own_session_and_records_it_first() {
    assert!(SCRIPT.contains("setsid sh -c"));
    let pid = SCRIPT.find("> \"$f.pid\"").expect("pid file");
    let rm = SCRIPT.find("rm -f \"$job\"").expect("rm job");
    assert!(pid < rm, "the pid is written before the job goes");
}

#[test]
fn night_script_waits_while_a_day_run_is_going() {
    assert!(SCRIPT.contains("busy()"));
    assert!(SCRIPT.contains("kill -0"));
}

#[test]
fn night_script_passes_sh_syntax() {
    let path = std::env::temp_dir().join(format!("binder-night-{}.sh", std::process::id()));
    std::fs::write(&path, SCRIPT).unwrap();
    let ok = std::process::Command::new("sh").arg("-n").arg(&path).status().unwrap().success();
    std::fs::remove_file(&path).ok();
    assert!(ok);
}

#[test]
fn stop_on_a_queued_run_removes_its_job_and_the_night_mark() {
    let (db, t) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    let r = FakeRemote::new(vec![Ok(String::new())]);
    let stopped = binder_core::agent::stop_run(&db, &r, &run, 200).unwrap();
    assert!(stopped.ended.is_some() && !stopped.queued);
    assert!(r.seen.borrow()[0].0.contains(&format!("night/'{run}.job'")), "{:?}", r.seen.borrow());
    assert!(!r.seen.borrow()[0].0.contains("kill"));
    let task = db.lock().unwrap().get_task(t.id).unwrap();
    assert!(!task.night);
    // The next round queues nothing.
    let r2 = FakeRemote::new(vec![Ok("--binder:jobs--\n".into())]);
    night::sync(&db, &r2, 300).unwrap();
    assert_eq!(r2.seen.borrow().len(), 1);
}

#[test]
fn stop_on_a_started_night_run_clears_the_night_mark() {
    let (db, t) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    poll_run(&db, &FakeRemote::new(vec![Ok("--binder:state--\nstarted\n".into()), Ok("--binder:state--\nrunning\n--binder:tail--\n\n".into())]), &run, 10_800).unwrap();
    let r = FakeRemote::new(vec![Ok(String::new())]);
    binder_core::agent::stop_run(&db, &r, &run, 11_000).unwrap();
    assert!(r.seen.borrow()[0].0.contains("kill -- -"));
    assert!(!db.lock().unwrap().get_task(t.id).unwrap().night);
}

#[test]
fn sync_keeps_a_dropped_run_whose_job_already_started() {
    let (db, t) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = queued(&db)[0].run.clone();
    db.lock().unwrap().update_task(t.id, TaskPatch { night: Some(false), ..Default::default() }, 150).unwrap();
    // night.sh already took the job: it is no longer on the server.
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into())]), 200).unwrap();
    assert_eq!(queued(&db).iter().map(|r| r.run.clone()).collect::<Vec<_>>(), vec![run]);
}

#[test]
fn clear_keeps_a_run_night_sh_started() {
    let (db, _) = night_task("~/code/orbit");
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let r = FakeRemote::new(vec![Ok("--binder:jobs--\n".into())]);
    night::clear(&db, &r).unwrap();
    assert_eq!(queued(&db).len(), 1, "a run whose job is gone is left for the poll to read");
}

#[test]
fn cron_install_refuses_when_the_crontab_cant_be_read() {
    let r = FakeRemote::new(vec![Ok("--binder:crontab--\n--binder:crontab-failed--\n".into())]);
    assert!(night::cron_install(&r).is_err());
    assert_eq!(r.seen.borrow().len(), 1, "nothing is written");
}

#[test]
fn cron_install_refuses_when_the_script_isnt_written() {
    let r = FakeRemote::new(vec![Ok("--binder:script-failed--\n--binder:crontab--\n5 * * * * backup\n".into())]);
    assert!(night::cron_install(&r).is_err());
    assert_eq!(r.seen.borrow().len(), 1);
}

#[test]
fn cron_remove_refuses_when_the_crontab_cant_be_read() {
    let r = FakeRemote::new(vec![Ok("--binder:crontab--\n--binder:crontab-failed--\n".into())]);
    assert!(night::cron_remove(&r).is_err());
    assert_eq!(r.seen.borrow().len(), 1);
}

#[test]
fn a_crontab_read_without_its_marker_is_an_error() {
    assert!(night::cron_state(&FakeRemote::new(vec![Ok(String::new())])).is_err());
    let r = FakeRemote::new(vec![Ok("motd only\n".into())]);
    assert!(night::cron_install(&r).is_err());
    assert_eq!(r.seen.borrow().len(), 1);
}

#[test]
fn cron_writes_keep_a_backup() {
    let r = FakeRemote::new(vec![Ok("--binder:crontab--\nMAILTO=me\n".into()), Ok(String::new())]);
    night::cron_install(&r).unwrap();
    assert!(r.seen.borrow()[1].0.contains("crontab.bak"));
}
