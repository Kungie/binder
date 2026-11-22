use binder_core::friends::*;
use binder_core::remote::FakeRemote;

#[test]
fn reads_the_list() {
    let r = FakeRemote::new(vec![Ok(r#"[{"name":"ali","created":1000,"last_login":null},{"name":"ece","created":2000,"last_login":3000}]"#.into())]);
    let list = list(&r).unwrap();
    assert_eq!(list, vec![Friend { name: "ali".into(), created: 1000, last_login: None }, Friend { name: "ece".into(), created: 2000, last_login: Some(3000) }]);
    assert!(r.seen.borrow()[0].0.contains("server.py users"));
}

#[test]
fn a_missing_service_reads_as_not_installed() {
    let r = FakeRemote::new(vec![Ok("--binder:missing--\n".into())]);
    assert_eq!(status(&r).unwrap(), Service { installed: false, configured: false, users: 0 });
    let r = FakeRemote::new(vec![Ok(r#"{"configured": true, "users": 2}"#.into())]);
    assert_eq!(status(&r).unwrap(), Service { installed: true, configured: true, users: 2 });
}

#[test]
fn passwords_and_secrets_travel_on_stdin_only() {
    let r = FakeRemote::new(vec![Ok(String::new()), Ok(String::new()), Ok(String::new())]);
    add(&r, "ali", "long enough").unwrap();
    set_password(&r, "ali", "another one").unwrap();
    set_oauth(&r, "kid", "tskey-client-secret").unwrap();
    let seen = r.seen.borrow();
    assert!(seen[0].0.contains("server.py add 'ali'"));
    assert_eq!(seen[0].1.as_deref(), Some("long enough\n"));
    assert!(seen[1].0.contains("server.py password 'ali'"));
    assert_eq!(seen[2].1.as_deref(), Some("kid\ntskey-client-secret\n"));
    for (script, _) in seen.iter() {
        assert!(!script.contains("long enough") && !script.contains("another one") && !script.contains("tskey"));
    }
}

#[test]
fn bad_input_never_reaches_the_server() {
    let r = FakeRemote::new(vec![]);
    for name in ["", "Ali", "-a", "a b", "a'b", &"a".repeat(33)] {
        assert!(add(&r, name, "long enough").is_err());
        assert!(remove(&r, name).is_err());
    }
    assert!(add(&r, "ali", "short").is_err());
    assert!(add(&r, "ali", "line\nbreak").is_err());
    assert!(set_oauth(&r, "", "x").is_err());
    assert!(set_oauth(&r, "a\nb", "x").is_err());
    assert!(r.seen.borrow().is_empty());
}

#[test]
fn remove_runs_remove() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    remove(&r, "ali").unwrap();
    assert!(r.seen.borrow()[0].0.contains("server.py remove 'ali'"));
}
