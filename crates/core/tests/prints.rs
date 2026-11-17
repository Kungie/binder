use binder_core::Db;
use binder_core::printer::{PrintOutcome, history};

#[test]
fn ended_prints_are_kept_newest_first() {
    let mut db = Db::in_memory().unwrap();
    history::record(&mut db, 100, &PrintOutcome::Done { job: "case-lid".into(), minutes: Some(95) }).unwrap();
    history::record(&mut db, 200, &PrintOutcome::Failed { job: "".into(), layer: Some(12) }).unwrap();
    let list = history::list(&db, 10).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].job.as_str(), list[0].ok, list[0].layer, list[0].at), ("Print", false, Some(12), 200));
    assert_eq!((list[1].job.as_str(), list[1].ok, list[1].minutes), ("case-lid", true, Some(95)));
    assert_eq!(history::list(&db, 1).unwrap().len(), 1);
}

#[test]
fn a_week_of_prints_sums_up() {
    let mut db = Db::in_memory().unwrap();
    for (at, m) in [(10, 60), (20, 125)] {
        history::record(&mut db, at, &PrintOutcome::Done { job: "x".into(), minutes: Some(m) }).unwrap();
    }
    history::record(&mut db, 30, &PrintOutcome::Failed { job: "y".into(), layer: None }).unwrap();
    history::record(&mut db, 99, &PrintOutcome::Done { job: "later".into(), minutes: Some(5) }).unwrap();
    assert_eq!(history::summary(&db, 0, 50).unwrap().as_deref(), Some("3 prints, 3 h 5 min, 1 failed"));
    assert_eq!(history::summary(&db, 1000, 2000).unwrap(), None);
}
