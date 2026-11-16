use binder_core::Db;
use binder_core::model::*;
use chrono::NaiveDate;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

#[test]
fn each_rule_steps_to_its_next_day() {
    let today = d("2026-11-16"); // a Monday
    assert_eq!(next_due(Repeat::Daily, Some(d("2026-11-16")), today), d("2026-11-17"));
    assert_eq!(next_due(Repeat::Weekly, Some(d("2026-11-16")), today), d("2026-11-23"));
    assert_eq!(next_due(Repeat::Monthly, Some(d("2026-11-16")), today), d("2026-12-16"));
    assert_eq!(next_due(Repeat::Yearly, Some(d("2026-11-16")), today), d("2027-11-16"));
    // Friday's next weekday is Monday.
    assert_eq!(next_due(Repeat::Weekdays, Some(d("2026-11-20")), d("2026-11-20")), d("2026-11-23"));
}

#[test]
fn a_month_without_the_day_takes_its_last() {
    assert_eq!(next_due(Repeat::Monthly, Some(d("2027-01-31")), d("2027-01-31")), d("2027-02-28"));
}

#[test]
fn a_late_finish_skips_to_after_today_and_an_early_one_keeps_the_rhythm() {
    // Three weeks overdue: no pile of copies, just the next one after today.
    assert_eq!(next_due(Repeat::Weekly, Some(d("2026-10-26")), d("2026-11-16")), d("2026-11-23"));
    // Done early: the next one is a week after the one it was due.
    assert_eq!(next_due(Repeat::Weekly, Some(d("2026-11-20")), d("2026-11-16")), d("2026-11-27"));
    // No due date: from today.
    assert_eq!(next_due(Repeat::Monthly, None, d("2026-11-16")), d("2026-12-16"));
}

#[test]
fn repeat_reads_back_as_it_was_written() {
    for r in [Repeat::Daily, Repeat::Weekdays, Repeat::Weekly, Repeat::Monthly, Repeat::Yearly] {
        assert_eq!(Repeat::parse(r.as_str()), Some(r));
    }
    assert_eq!(Repeat::parse("hourly"), None);
}

/// 2026-11-16 12:00 UTC, midday everywhere the owner might be.
const NOW: i64 = 1_794_830_400;

#[test]
fn finishing_a_repeating_task_brings_the_next_one() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "home".into(), ..Default::default() }, 1).unwrap();
    let t = db
        .create_task(
            NewTask {
                title: "Pay rent".into(),
                notes: Some("Bank transfer".into()),
                priority: Some(Priority::High),
                project_id: Some(p.id),
                due: Some("2026-12-01".into()),
                due_time: Some("09:00".into()),
                repeat: Some(Repeat::Monthly),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
    assert_eq!(t.repeat, Some(Repeat::Monthly));
    let done = db.update_task(t.id, TaskPatch { status: Some(Status::Done), ..Default::default() }, NOW + 10).unwrap();
    // The finished one stops repeating, so reopening and finishing it again brings no second copy.
    assert_eq!((done.status, done.repeat), (Status::Done, None));
    let open = db.list_tasks(TaskView::All, false).unwrap();
    assert_eq!(open.len(), 1);
    let next = &open[0];
    assert_ne!(next.id, t.id);
    assert_eq!(
        (next.title.as_str(), next.notes.as_str(), next.priority, next.project_id, next.status),
        ("Pay rent", "Bank transfer", Priority::High, Some(p.id), Status::Todo)
    );
    assert_eq!((next.due.as_deref(), next.due_time.as_deref(), next.repeat), (Some("2027-01-01"), Some("09:00"), Some(Repeat::Monthly)));
    db.update_task(t.id, TaskPatch { status: Some(Status::Todo), ..Default::default() }, NOW + 20).unwrap();
    db.update_task(t.id, TaskPatch { status: Some(Status::Done), ..Default::default() }, NOW + 30).unwrap();
    assert_eq!(db.list_tasks(TaskView::All, false).unwrap().len(), 1);
}

#[test]
fn canceling_a_repeating_task_ends_it() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(NewTask { title: "Backup".into(), repeat: Some(Repeat::Weekly), ..Default::default() }, NOW).unwrap();
    db.update_task(t.id, TaskPatch { status: Some(Status::Canceled), ..Default::default() }, NOW + 10).unwrap();
    assert!(db.list_tasks(TaskView::All, false).unwrap().is_empty());
}

#[test]
fn repeat_can_be_set_and_cleared() {
    let mut db = Db::in_memory().unwrap();
    let t = db.create_task(NewTask { title: "Water plants".into(), ..Default::default() }, NOW).unwrap();
    assert_eq!(t.repeat, None);
    let t = db.update_task(t.id, TaskPatch { repeat: Some(Repeat::Weekly), ..Default::default() }, NOW).unwrap();
    assert_eq!(t.repeat, Some(Repeat::Weekly));
    let t = db.update_task(t.id, TaskPatch { clear_repeat: true, ..Default::default() }, NOW).unwrap();
    assert_eq!(t.repeat, None);
}
