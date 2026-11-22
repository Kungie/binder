use binder_core::friends::*;
use binder_core::remote::FakeRemote;

#[test]
fn reads_the_list() {
    let r = FakeRemote::new(vec![Ok(r#"[{"name":"ali","created":1000,"last_login":null},{"name":"ece","created":2000,"last_login":3000}]"#.into())]);
    let list = list(&r, "binder-vpn").unwrap();
    assert_eq!(list, vec![Friend { name: "ali".into(), created: 1000, last_login: None }, Friend { name: "ece".into(), created: 2000, last_login: Some(3000) }]);
    assert!(r.seen.borrow()[0].0.contains("server.py users"));
}

#[test]
fn a_missing_service_reads_as_not_installed() {
    let r = FakeRemote::new(vec![Ok("--binder:missing--\n".into())]);
    assert_eq!(status(&r, "binder-vpn").unwrap(), Service { installed: false, configured: false, users: 0 });
    let r = FakeRemote::new(vec![Ok(r#"{"configured": true, "users": 2}"#.into())]);
    assert_eq!(status(&r, "binder-vpn").unwrap(), Service { installed: true, configured: true, users: 2 });
}

#[test]
fn passwords_and_secrets_travel_on_stdin_only() {
    let r = FakeRemote::new(vec![Ok(String::new()), Ok(String::new()), Ok(String::new())]);
    add(&r, "binder-vpn", "ali", "long enough").unwrap();
    set_password(&r, "binder-vpn", "ali", "another one").unwrap();
    set_oauth(&r, "binder-vpn", "kid", "tskey-client-secret").unwrap();
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
        assert!(add(&r, "binder-vpn", name, "long enough").is_err());
        assert!(remove(&r, "binder-vpn", name).is_err());
    }
    assert!(add(&r, "binder-vpn", "ali", "short").is_err());
    assert!(add(&r, "binder-vpn", "ali", "line\nbreak").is_err());
    assert!(set_oauth(&r, "binder-vpn", "", "x").is_err());
    assert!(set_oauth(&r, "binder-vpn", "a\nb", "x").is_err());
    assert!(r.seen.borrow().is_empty());
}

#[test]
fn remove_runs_remove() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    remove(&r, "binder-vpn", "ali").unwrap();
    assert!(r.seen.borrow()[0].0.contains("server.py remove 'ali'"));
}

#[test]
fn the_folder_is_quoted_and_never_leaves_home() {
    let r = FakeRemote::new(vec![Ok("[]".into())]);
    list(&r, "sites/binder-site").unwrap();
    assert!(r.seen.borrow()[0].0.contains("cd \"$HOME\"/'sites/binder-site'"));
    for bad in ["", "/etc", "../x", "a/../../b", "a b", "a;b", "a'b", "$(x)", "a\nb"] {
        assert!(list(&r, bad).is_err(), "{bad}");
        assert!(install(&r, bad).is_err(), "{bad}");
    }
    assert_eq!(r.seen.borrow().len(), 1);
}

#[test]
fn install_writes_the_service_and_starts_it() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    install(&r, "binder-vpn").unwrap();
    let (script, stdin) = r.seen.borrow()[0].clone();
    assert!(script.contains("mkdir -p"));
    assert!(script.contains("docker compose up -d --build vpn"));
    // A compose file the owner already has is kept.
    assert!(script.contains("[ -e docker-compose.yml ]"));
    let stdin = stdin.unwrap();
    assert!(stdin.contains("def login("));
    assert!(stdin.contains("FROM python:3.13-alpine"));
    assert!(stdin.contains("127.0.0.1:8787:8080"));
    assert!(stdin.contains("read_only: true"));
}

#[test]
fn an_address_is_https_only() {
    assert_eq!(address("https://vpn.example.com/").unwrap(), "https://vpn.example.com");
    assert_eq!(address(" https://lab.tail1234.ts.net ").unwrap(), "https://lab.tail1234.ts.net");
    for bad in ["http://vpn.example.com", "vpn.example.com", "https://", "https://a b", "https://a\nb", "-https://x", "https://x/\"y"] {
        assert!(address(bad).is_err(), "{bad}");
    }
}
