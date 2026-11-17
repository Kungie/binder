use binder_core::agent::{Access, poll_run, start_run};
use binder_core::brief::{self, BriefItem};
use binder_core::events::EventKind;
use binder_core::model::*;
use binder_core::night;
use binder_core::remote::FakeRemote;
use binder_core::Db;
use chrono::NaiveDate;
use std::sync::Mutex;

fn db_with_project() -> (Mutex<Db>, Project) {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    db.update_project(p.id, ProjectPatch { agent_dir: Some("~/orbit".into()), ..Default::default() }).unwrap();
    (Mutex::new(db), p)
}

/// A day run that ends at `ended` with `code`.
fn day_run(db: &Mutex<Db>, p: &Project, title: &str, ended: i64, code: i32) -> Task {
    let t = db.lock().unwrap().create_task(NewTask { title: title.into(), project_id: Some(p.id), assignee: Some(Assignee::Agent), ..Default::default() }, 1).unwrap();
    let r = FakeRemote::new(vec![Ok(String::new()), Ok(format!("exit:{code}\n--binder:tail--\n"))]);
    let run = start_run(db, &r, t.id, "~/orbit", Access::EditsOnly, ended - 100).unwrap();
    poll_run(db, &r, &run.run, ended).unwrap();
    t
}

#[test]
fn runs_containers_prints_and_issues_come_in_order() {
    let (db, p) = db_with_project();
    let a = day_run(&db, &p, "Ship", 1500, 0);
    let b = day_run(&db, &p, "Break", 1600, 1);
    {
        let mut d = db.lock().unwrap();
        d.log_event(1100, EventKind::ServerRestart, "", "").unwrap();
        d.log_event(1200, EventKind::PrintDone, "Benchy", "Took 38 min").unwrap();
        d.log_event(1300, EventKind::PrintFailed, "Vase", "Stopped at layer 112").unwrap();
        let t = d.create_task(NewTask { title: "Crash on start".into(), project_id: Some(p.id), ..Default::default() }, 1).unwrap();
        d.log_event(1700, EventKind::IssueAdded, &t.key, "Crash on start").unwrap();
    }
    let d = db.lock().unwrap();
    let items = brief::items(&d, 1000, 2000).unwrap();
    let issue = d.list_tasks(TaskView::All, true).unwrap().into_iter().find(|t| t.title == "Crash on start").unwrap();
    assert_eq!(
        items,
        vec![
            BriefItem::ServerRestart { at: 1100 },
            BriefItem::Print { job: "Benchy".into(), ok: true, at: 1200 },
            BriefItem::Print { job: "Vase".into(), ok: false, at: 1300 },
            BriefItem::Run { task_id: a.id, key: a.key.clone(), ok: true, night: false, at: 1500 },
            BriefItem::Run { task_id: b.id, key: b.key.clone(), ok: false, night: false, at: 1600 },
            BriefItem::Issue { task_id: issue.id, key: issue.key.clone(), title: "Crash on start".into(), at: 1700 },
        ]
    );
    // Outside the range, nothing.
    assert!(brief::items(&d, 2000, 3000).unwrap().is_empty());
}

#[test]
fn night_runs_say_so() {
    let (db, p) = db_with_project();
    let t = db.lock().unwrap().create_task(NewTask { title: "Rotate".into(), project_id: Some(p.id), night: Some(true), ..Default::default() }, 1).unwrap();
    night::sync(&db, &FakeRemote::new(vec![Ok("--binder:jobs--\n".into()), Ok(String::new())]), 100).unwrap();
    let run = binder_core::agent::list_runs(&db.lock().unwrap(), t.id).unwrap()[0].run.clone();
    let r = FakeRemote::new(vec![Ok("--binder:state--\nstarted\n".into()), Ok("exit:0\n--binder:tail--\n".into())]);
    poll_run(&db, &r, &run, 10_800).unwrap();
    let d = db.lock().unwrap();
    assert_eq!(brief::items(&d, 0, 20_000).unwrap(), vec![BriefItem::Run { task_id: t.id, key: t.key.clone(), ok: true, night: true, at: 10_800 }]);
}

#[test]
fn a_container_shows_its_latest_problem_and_when_it_came_back() {
    let (db, _) = db_with_project();
    let mut d = db.lock().unwrap();
    d.log_event(100, EventKind::ContainerUnhealthy, "web", "").unwrap();
    d.log_event(150, EventKind::ContainerBack, "web", "").unwrap();
    d.log_event(200, EventKind::ContainerStopped, "web", "").unwrap();
    d.log_event(260, EventKind::ContainerBack, "web", "").unwrap();
    d.log_event(300, EventKind::ContainerRestarting, "db", "").unwrap();
    assert_eq!(
        brief::items(&d, 0, 1000).unwrap(),
        vec![
            BriefItem::Container { name: "web".into(), problem: "stopped".into(), at: 200, back_at: Some(260) },
            BriefItem::Container { name: "db".into(), problem: "restarting".into(), at: 300, back_at: None },
        ]
    );
}

#[test]
fn owner_restarts_stay_out() {
    let (db, _) = db_with_project();
    let mut d = db.lock().unwrap();
    d.log_event(100, EventKind::OwnerRestart, "web", "").unwrap();
    assert!(brief::items(&d, 0, 1000).unwrap().is_empty());
}

#[test]
fn since_is_the_last_dismiss_or_a_day() {
    let mut d = Db::in_memory().unwrap();
    assert_eq!(brief::since(&d, 100_000).unwrap(), 100_000 - 86_400);
    brief::dismiss(&mut d, 90_000).unwrap();
    assert_eq!(brief::since(&d, 100_000).unwrap(), 90_000);
}

#[test]
fn shown_from_five_until_noon_until_dismissed() {
    let day = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
    let at = |h: u32, m: u32| day.and_hms_opt(h, m, 0).unwrap();
    assert!(!brief::shown(at(4, 59), None));
    assert!(brief::shown(at(5, 0), None));
    assert!(brief::shown(at(11, 59), None));
    assert!(!brief::shown(at(12, 0), None));
    assert!(!brief::shown(at(9, 0), Some(at(6, 0))));
    let yesterday = NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(9, 0, 0).unwrap();
    assert!(brief::shown(at(9, 0), Some(yesterday)));
    // Dismissed today before 05:00 (late last night) still shows this morning's.
    assert!(brief::shown(at(9, 0), Some(at(1, 0))));
}

#[test]
fn the_brief_has_sites_that_went_down_and_full_disks() {
    use binder_core::brief::{BriefItem, items};
    use binder_core::events::EventKind;
    let mut db = binder_core::Db::in_memory().unwrap();
    db.log_event(100, EventKind::SiteDown, "blog", "").unwrap();
    db.log_event(400, EventKind::SiteBack, "blog", "").unwrap();
    db.log_event(500, EventKind::DiskFull, "Server disk", "92").unwrap();
    let got = items(&db, 0, 1000).unwrap();
    assert_eq!(
        got,
        vec![BriefItem::Site { name: "blog".into(), at: 100, back_at: Some(400) }, BriefItem::Disk { name: "Server disk".into(), percent: 92, at: 500 }]
    );
}
