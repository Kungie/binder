use binder_core::server::Container;
use binder_core::watch::{Alert, Watch};

fn c(name: &str, state: &str, status: &str) -> Container {
    Container { name: name.into(), image: "img".into(), state: state.into(), status: status.into() }
}

#[test]
fn already_stopped_at_start_is_quiet() {
    let mut w = Watch::default();
    assert!(w.seen(&[c("old", "exited", "Exited (0) 3 days ago")]).is_empty());
    assert!(w.seen(&[c("old", "exited", "Exited (0) 3 days ago")]).is_empty());
}

#[test]
fn running_then_exited_alerts_once() {
    let mut w = Watch::default();
    assert!(w.seen(&[c("web", "running", "Up 2 hours")]).is_empty());
    assert_eq!(w.seen(&[c("web", "exited", "Exited (137) 5 seconds ago")]), vec![Alert::Stopped("web".into(), "Exited (137) 5 seconds ago".into())]);
    assert!(w.seen(&[c("web", "exited", "Exited (137) 1 minute ago")]).is_empty());
}

#[test]
fn one_alert_per_change() {
    let mut w = Watch::default();
    w.seen(&[c("db", "running", "Up 1 hour")]);
    let mut all = vec![];
    for (state, status) in [("exited", "Exited (1) now"), ("exited", "Exited (1) 1 min"), ("running", "Up 3 seconds"), ("running", "Up 1 minute"), ("running", "Up 2 minutes")] {
        all.extend(w.seen(&[c("db", state, status)]));
    }
    assert_eq!(all, vec![Alert::Stopped("db".into(), "Exited (1) now".into()), Alert::Running("db".into())]);
}

#[test]
fn a_container_that_disappears_counts_as_stopped() {
    let mut w = Watch::default();
    w.seen(&[c("web", "running", "Up 2 hours")]);
    assert_eq!(w.seen(&[]), vec![Alert::Stopped("web".into(), "Removed".into())]);
}

#[test]
fn unhealthy_and_restarting() {
    let mut w = Watch::default();
    let first = w.seen(&[c("a", "running", "Up 5 minutes (unhealthy)"), c("b", "restarting", "Restarting (1) 4 seconds ago"), c("c", "running", "Up 1 hour (healthy)")]);
    assert_eq!(first, vec![Alert::Unhealthy("a".into(), "Up 5 minutes (unhealthy)".into()), Alert::Restarting("b".into(), "Restarting (1) 4 seconds ago".into())]);
    let later = [c("a", "running", "Up 6 minutes (healthy)"), c("b", "restarting", "Restarting (1) 1 second ago"), c("c", "running", "Up 1 hour (healthy)")];
    assert!(w.seen(&later).is_empty());
    assert!(w.seen(&later).is_empty());
    assert_eq!(w.seen(&later), vec![Alert::Running("a".into())], "back after three clean checks");
}

#[test]
fn three_misses_then_back() {
    let mut w = Watch::default();
    w.seen(&[c("web", "running", "Up")]);
    assert!(w.missed().is_empty());
    assert!(w.missed().is_empty());
    assert_eq!(w.missed(), vec![Alert::Unreachable]);
    assert!(w.missed().is_empty());
    assert_eq!(w.seen(&[c("web", "running", "Up")]), vec![Alert::Back]);
    assert!(w.missed().is_empty(), "the count starts again");
}

#[test]
fn titles_read_as_the_spec_says() {
    assert_eq!(Alert::Stopped("web".into(), "Exited (1)".into()).title(), "web stopped");
    assert_eq!(Alert::Restarting("web".into(), "s".into()).title(), "web keeps restarting");
    assert_eq!(Alert::Unhealthy("web".into(), "s".into()).title(), "web is unhealthy");
    assert_eq!(Alert::Running("web".into()).title(), "web is running again");
    assert_eq!(Alert::Unreachable.title(), "Can't reach the server");
    assert_eq!(Alert::Back.title(), "The server is back");
    assert_eq!(Alert::Stopped("web".into(), "Exited (1)".into()).body(), "Exited (1)");
}

#[test]
fn a_crash_loop_alerts_once() {
    let mut w = Watch::default();
    w.seen(&[c("api", "running", "Up 2 hours")]);
    let mut all = vec![];
    for _ in 0..4 {
        for (state, status) in [("restarting", "Restarting (1) 2 seconds ago"), ("running", "Up 10 seconds"), ("exited", "Exited (1) 5 seconds ago")] {
            all.extend(w.seen(&[c("api", state, status)]));
        }
    }
    assert_eq!(all, vec![Alert::Restarting("api".into(), "Restarting (1) 2 seconds ago".into())]);
}

#[test]
fn a_removed_container_that_returns_says_so() {
    let mut w = Watch::default();
    w.seen(&[c("web", "running", "Up 2 hours")]);
    assert_eq!(w.seen(&[]), vec![Alert::Stopped("web".into(), "Removed".into())]);
    assert!(w.seen(&[]).is_empty());
    let up = [c("web", "running", "Up 1 minute")];
    assert!(w.seen(&up).is_empty());
    assert!(w.seen(&up).is_empty());
    assert_eq!(w.seen(&up), vec![Alert::Running("web".into())]);
}

use binder_core::events::EventKind;
use binder_core::server::ContainerTimes;
use binder_core::watch::{catch_up, rebooted};

fn times(name: &str, state: &str, started: Option<i64>, finished: Option<i64>) -> ContainerTimes {
    ContainerTimes { name: name.into(), state: state.into(), started_at: started, finished_at: finished, restarts: 0 }
}

#[test]
fn a_gap_reads_what_the_server_remembers() {
    // Last check 23:00, now 07:30; web stopped 02:14 and came back 02:20.
    let (last, now) = (23 * 3600, 31 * 3600 + 1800);
    let (stop, start) = (26 * 3600 + 14 * 60, 26 * 3600 + 20 * 60);
    let got = catch_up(last, now, &[times("web", "running", Some(start), Some(stop)), times("db", "running", Some(100), None)], &[]);
    assert_eq!(got, vec![(stop, EventKind::ContainerStopped, "web".to_string()), (start, EventKind::ContainerBack, "web".to_string())]);
}

#[test]
fn still_stopped_logs_only_the_stop() {
    let got = catch_up(0, 1000, &[times("web", "exited", Some(10), Some(500))], &[]);
    assert_eq!(got, vec![(500, EventKind::ContainerStopped, "web".to_string())]);
}

#[test]
fn a_fresh_start_without_a_stop_is_not_news() {
    // A container created during the gap (a deploy) started but never stopped there.
    assert!(catch_up(0, 1000, &[times("new", "running", Some(400), None)], &[]).is_empty());
}

#[test]
fn the_same_gap_is_not_logged_twice() {
    let t = [times("web", "running", Some(600), Some(500))];
    assert_eq!(catch_up(0, 1000, &t, &[]).len(), 2);
    assert!(catch_up(1000, 2000, &t, &[]).is_empty());
}

#[test]
fn an_owner_restart_is_quiet() {
    let t = [times("web", "running", Some(510), Some(505))];
    assert!(catch_up(0, 1000, &t, &[("web".into(), 500)]).is_empty());
    let mut w = Watch::default();
    w.seen(&[c("web", "running", "Up 2 hours")]);
    let quiet = vec!["web".to_string()];
    assert!(w.seen_with(&[c("web", "exited", "Exited (0) now")], &quiet).is_empty());
    assert!(w.seen_with(&[c("web", "running", "Up 1 second")], &quiet).is_empty());
    assert!(w.seen(&[c("web", "running", "Up 1 minute")]).is_empty());
}

#[test]
fn rebooted_needs_a_later_boot() {
    assert_eq!(rebooted(None, Some(1000)), None);
    assert_eq!(rebooted(Some(1000), Some(1030)), None);
    assert_eq!(rebooted(Some(1000), Some(5000)), Some(5000));
    assert_eq!(rebooted(Some(1000), None), None);
}

#[test]
fn alerts_name_their_event_kind() {
    assert_eq!(Alert::Stopped("a".into(), "".into()).kind(), Some(EventKind::ContainerStopped));
    assert_eq!(Alert::Running("a".into()).kind(), Some(EventKind::ContainerBack));
    assert_eq!(Alert::Unreachable.kind(), None);
}

#[test]
fn a_catch_up_sees_an_owner_restart_from_before_the_gap() {
    use binder_core::watch::owner_since;
    // Last good check 23:00:00, now 07:30: the restart at 22:59:30 still counts for the gap.
    let (last, now) = (23 * 3600, 31 * 3600 + 1800);
    assert!(owner_since(Some(last), now) <= last - 30);
    // Without a gap the window is the last two minutes.
    assert_eq!(owner_since(Some(now - 60), now), now - 120);
    assert_eq!(owner_since(None, now), now - 120);
}

#[test]
fn a_stop_the_catch_up_logged_is_not_logged_again_live() {
    use binder_core::watch::live_events;
    let caught = vec![(500, EventKind::ContainerStopped, "web".to_string())];
    let alerts = vec![Alert::Stopped("web".into(), "Exited (1)".into()), Alert::Stopped("db".into(), "Exited (0)".into())];
    let got = live_events(&alerts, &caught, 1000);
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].0, got[0].1, got[0].2.as_str()), (1000, EventKind::ContainerStopped, "db"));
    // One that came back in the gap and stopped again since is news.
    let back = vec![(500, EventKind::ContainerStopped, "web".to_string()), (600, EventKind::ContainerBack, "web".to_string())];
    assert_eq!(live_events(&alerts[..1], &back, 1000).len(), 1);
}
