use binder_core::tailscale::parse_status;
use binder_core::vpn_client::*;

#[test]
fn reads_profiles_and_the_current_one() {
    let out = "ID    Tailnet             Account\n2c1f  kungie.github       kungie@github\n7a9d  demo-dev.ts.net     tagged-devices*\n";
    let p = parse_profiles(out);
    assert_eq!(p.len(), 2);
    assert_eq!(p[1], Profile { id: "7a9d".into(), tailnet: "demo-dev.ts.net".into(), account: "tagged-devices".into(), current: true });
    assert!(!p[0].current);
    assert_eq!(current_profile(out).as_deref(), Some("7a9d"));
    assert!(parse_profiles("").is_empty());
}

#[test]
fn a_sign_in_answer_gives_the_key_or_the_services_words() {
    let ok = parse_login(r#"{"key":"tskey-auth-abc","hostname":"friend-ali"}"#, 200).unwrap();
    assert_eq!(ok, Login { key: "tskey-auth-abc".into(), hostname: "friend-ali".into(), token: None });
    let with_token = parse_login(r#"{"key":"tskey-auth-abc","hostname":"friend-ali","token":"t0k-en_1"}"#, 200).unwrap();
    assert_eq!(with_token.token.as_deref(), Some("t0k-en_1"));
    // A token the app would keep has to look like one.
    assert_eq!(parse_login(r#"{"key":"tskey-auth-abc","hostname":"friend-ali","token":"a b"}"#, 200).unwrap().token, None);
    assert_eq!(parse_login(r#"{"error":"Wrong user name or password."}"#, 401).unwrap_err(), "Wrong user name or password.");
    assert_eq!(parse_login("<html>", 502).unwrap_err(), "Binder VPN's service didn't answer. Try again later.");
    // A host name the app will pass to Tailscale is checked, whatever the service says.
    assert!(parse_login(r#"{"key":"tskey-auth-abc","hostname":"--evil"}"#, 200).is_err());
    assert!(parse_login(r#"{"key":"x y","hostname":"friend-ali"}"#, 200).is_err());
}

#[test]
fn the_request_is_json_with_the_password_escaped() {
    let body = login_body("ali", "p\"ss\\word");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["user"], "ali");
    assert_eq!(v["password"], "p\"ss\\word");
}

#[test]
fn turning_on_resets_to_this_host_and_exit_node() {
    assert_eq!(
        up_args("friend-ali", "100.64.0.4"),
        ["up", "--reset", "--hostname=friend-ali", "--exit-node=100.64.0.4", "--exit-node-allow-lan-access"]
    );
}

#[test]
fn regions_are_exit_nodes_named_by_city() {
    let json = r#"{"BackendState":"Running","Self":{"ID":"me","HostName":"mac","Relay":"ams","TailscaleIPs":["100.64.0.9"]},
      "Peer":{"a":{"ID":"a","HostName":"vps-germany","Relay":"fra","TailscaleIPs":["100.64.0.4"],"Online":true,"ExitNodeOption":true,"ExitNode":true},
              "b":{"ID":"b","HostName":"vps-sg","Relay":"sin","TailscaleIPs":["100.64.0.5"],"Online":false,"ExitNodeOption":true},
              "c":{"ID":"c","HostName":"lab","Relay":"fra","TailscaleIPs":["100.64.0.6"],"Online":true}}}"#;
    let r = regions(&parse_status(json).unwrap());
    assert_eq!(r.len(), 2);
    assert_eq!(r[0], Region { name: "vps-germany".into(), ip: "100.64.0.4".into(), city: "Frankfurt".into(), online: true, active: true });
    assert_eq!(r[1].city, "Singapore");
    assert!(!r[1].online);
    assert_eq!(city("zzz"), None);
}

#[test]
fn the_sign_in_address_is_the_servers_https_address() {
    assert_eq!(login_url("https://vpn.example.com/").unwrap(), "https://vpn.example.com/vpn/api/login");
    assert!(login_url("http://vpn.example.com").is_err());
    assert!(login_url("-K /etc/passwd").is_err());
    assert!(login_url("").is_err());
}

#[test]
fn the_menu_says_how_far_an_update_is() {
    assert_eq!(update_word(None), "Updating");
    assert_eq!(update_word(Some(42)), "Updating 42%");
    assert_eq!(percent(421, Some(1000)), Some(42));
    assert_eq!(percent(1200, Some(1000)), Some(100));
    assert_eq!(percent(5, None), None);
    assert_eq!(percent(5, Some(0)), None);
}

#[test]
fn signing_out_sends_the_token_and_this_devices_id() {
    assert_eq!(logout_url("https://vpn.example.com").unwrap(), "https://vpn.example.com/vpn/api/logout");
    let v: serde_json::Value = serde_json::from_str(&logout_body("ali", "tok", "nABC")).unwrap();
    assert_eq!((v["user"].as_str(), v["token"].as_str(), v["node"].as_str()), (Some("ali"), Some("tok"), Some("nABC")));
    let json = r#"{"BackendState":"Running","Self":{"ID":"nSELF","HostName":"mac","TailscaleIPs":["100.64.0.9"]},"Peer":{"a":{"ID":"nPEER","HostName":"x"}}}"#;
    assert_eq!(self_id(&parse_status(json).unwrap()).as_deref(), Some("nSELF"));
}
