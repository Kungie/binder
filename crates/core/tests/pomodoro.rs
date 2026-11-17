use binder_core::pomodoro::{Phase, Pomodoro, FOCUS, LONG_BREAK, SHORT_BREAK};

#[test]
fn focus_runs_down_and_shows_minutes_and_seconds() {
    let mut p = Pomodoro::default();
    assert_eq!(p.label(0), None);
    p.focus(1000);
    assert_eq!(p.phase(), Some(Phase::Focus));
    assert_eq!(p.label(1000).as_deref(), Some("25:00"));
    assert_eq!(p.label(1000 + 47).as_deref(), Some("24:13"));
}

#[test]
fn a_finished_focus_starts_a_break_and_every_fourth_is_long() {
    let mut p = Pomodoro::default();
    let mut t = 0;
    for n in 1..=4 {
        p.focus(t);
        assert_eq!(p.tick(t + FOCUS - 1), None);
        t += FOCUS;
        let want = if n == 4 { Phase::LongBreak } else { Phase::ShortBreak };
        assert_eq!(p.tick(t), Some((Phase::Focus, Some(want))));
        assert_eq!(p.phase(), Some(want));
        assert_eq!(p.done(), n);
        t += if n == 4 { LONG_BREAK } else { SHORT_BREAK };
        // A break that ends waits for the owner to start the next focus.
        assert_eq!(p.tick(t), Some((want, None)));
        assert_eq!(p.phase(), None);
    }
}

#[test]
fn pause_holds_the_time_and_resume_carries_on() {
    let mut p = Pomodoro::default();
    p.focus(0);
    p.pause(60);
    assert!(p.paused());
    assert_eq!(p.label(10_000).as_deref(), Some("24:00"));
    assert_eq!(p.tick(10_000), None);
    p.resume(10_000);
    assert_eq!(p.label(10_060).as_deref(), Some("23:00"));
    p.stop();
    assert_eq!((p.phase(), p.label(10_060)), (None, None));
}

#[test]
fn skipping_a_break_ends_it_without_a_notice() {
    let mut p = Pomodoro::default();
    p.focus(0);
    p.tick(FOCUS);
    p.stop();
    assert_eq!(p.phase(), None);
    assert_eq!(p.done(), 1);
}

#[test]
fn finished_sessions_are_counted_from_a_time() {
    let mut db = binder_core::Db::in_memory().unwrap();
    binder_core::pomodoro::record(&mut db, 100, 25).unwrap();
    binder_core::pomodoro::record(&mut db, 200, 25).unwrap();
    assert_eq!(binder_core::pomodoro::count_since(&db, 150).unwrap(), 1);
}
