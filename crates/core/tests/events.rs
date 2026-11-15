use binder_core::Db;
use binder_core::events::EventKind;

#[test]
fn events_come_back_in_order_within_the_range() {
    let mut db = Db::in_memory().unwrap();
    db.log_event(30, EventKind::ContainerBack, "web", "").unwrap();
    db.log_event(10, EventKind::ContainerStopped, "web", "Exited (1)").unwrap();
    db.log_event(50, EventKind::ServerRestart, "", "").unwrap();
    let got = db.events_between(10, 50).unwrap();
    let kinds: Vec<_> = got.iter().map(|e| (e.at, e.kind)).collect();
    assert_eq!(kinds, vec![(10, EventKind::ContainerStopped), (30, EventKind::ContainerBack)]);
    assert_eq!((got[0].subject.as_str(), got[0].detail.as_str()), ("web", "Exited (1)"));
}

#[test]
fn prune_drops_only_older_events() {
    let mut db = Db::in_memory().unwrap();
    db.log_event(10, EventKind::PrintDone, "Benchy", "").unwrap();
    db.log_event(20, EventKind::PrintFailed, "Vase", "").unwrap();
    assert_eq!(db.prune_events(20).unwrap(), 1);
    let left = db.events_between(0, 100).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].kind, EventKind::PrintFailed);
}

#[test]
fn kinds_round_trip_through_text() {
    for k in [EventKind::ContainerStopped, EventKind::ContainerRestarting, EventKind::ContainerUnhealthy, EventKind::ContainerBack, EventKind::OwnerRestart, EventKind::ServerRestart, EventKind::PrintDone, EventKind::PrintFailed, EventKind::IssueAdded] {
        assert_eq!(EventKind::parse(k.as_str()), Some(k));
    }
}
