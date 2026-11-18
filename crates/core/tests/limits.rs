use binder_core::limits::*;
use binder_core::remote::FakeRemote;

const INPUT: &str = r#"{"session_id":"s","rate_limits":{"five_hour":{"used_percentage":34.4,"resets_at":1800000000},"seven_day":{"used_percentage":61,"resets_at":"2027-01-20T00:00:00Z"}}}"#;

#[test]
fn reads_both_windows_and_their_resets() {
    let l = parse(&format!("1799990000\n{INPUT}\n"), 1_799_990_100).unwrap();
    assert_eq!(l.read_at, 1_799_990_000);
    assert_eq!(l.five_hour, Some(Window { percent: 34.4, resets_at: Some(1_800_000_000) }));
    assert_eq!(l.seven_day, Some(Window { percent: 61.0, resets_at: Some(1_800_403_200) }));
}

#[test]
fn a_window_that_has_reset_since_reads_zero() {
    let l = parse(&format!("1799990000\n{INPUT}\n"), 1_800_000_001).unwrap();
    assert_eq!(l.five_hour, Some(Window { percent: 0.0, resets_at: None }));
    assert_eq!(l.seven_day.unwrap().percent, 61.0);
}

#[test]
fn nothing_kept_yet_is_none() {
    assert_eq!(parse("", 1), None);
    assert_eq!(parse("1799990000\n{\"session_id\":\"s\"}\n", 1), None);
}

#[test]
fn install_adds_the_status_line_and_keeps_the_rest() {
    let r = FakeRemote::new(vec![Ok(r#"{"theme":"dark","permissions":{"deny":["x"]}}"#.into()), Ok(String::new()), Ok(String::new())]);
    install(&r).unwrap();
    let seen = r.seen.borrow();
    assert_eq!(seen.len(), 3);
    let script = seen[1].1.as_deref().unwrap();
    assert!(script.contains("binder-limits.json"));
    assert!(!script.contains("sh -c"), "{script}");
    let settings: serde_json::Value = serde_json::from_str(seen[2].1.as_deref().unwrap()).unwrap();
    assert_eq!(settings["theme"], "dark");
    assert_eq!(settings["permissions"]["deny"][0], "x");
    assert_eq!(settings["statusLine"]["type"], "command");
    assert_eq!(settings["statusLine"]["command"], "sh ~/.claude/binder-statusline.sh");
    assert!(seen[2].0.contains(".binder-backup"));
}

#[test]
fn a_status_line_already_there_still_runs_after_binder_keeps_the_limits() {
    let r = FakeRemote::new(vec![Ok(r#"{"statusLine":{"type":"command","command":"~/bin/line 'a'"}}"#.into())]);
    install(&r).unwrap();
    let seen = r.seen.borrow();
    let script = seen[1].1.as_deref().unwrap();
    assert!(script.contains(r#"sh -c '~/bin/line '\''a'\'''"#), "{script}");
}

#[test]
fn installing_twice_changes_nothing() {
    let r = FakeRemote::new(vec![Ok(r#"{"statusLine":{"type":"command","command":"sh ~/.claude/binder-statusline.sh"}}"#.into())]);
    install(&r).unwrap();
    assert_eq!(r.seen.borrow().len(), 1);
    assert!(installed(r#"{"statusLine":{"type":"command","command":"sh ~/.claude/binder-statusline.sh"}}"#));
    assert!(!installed("{}"));
    assert!(!installed(""));
}
