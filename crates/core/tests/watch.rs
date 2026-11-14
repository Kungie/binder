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
