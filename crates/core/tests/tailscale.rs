use binder_core::tailscale::*;
use std::collections::HashMap;

const STATUS: &str = r#"{
  "BackendState": "Running",
  "Self": {"ID": "n1", "HostName": "demo-mac", "DNSName": "demo-mac.tail0.ts.net.", "OS": "macOS", "TailscaleIPs": ["100.64.0.1", "fd7a::1"], "Online": true, "KeyExpiry": "2027-01-10T00:00:00Z", "LastSeen": "0001-01-01T00:00:00Z", "Relay": "fra", "CurAddr": ""},
  "Peer": {
    "k2": {"ID": "n2", "HostName": "lab", "DNSName": "lab.tail0.ts.net.", "OS": "linux", "TailscaleIPs": ["100.64.0.2"], "Online": true, "Active": true, "Relay": "fra", "CurAddr": "", "LastSeen": "0001-01-01T00:00:00Z", "TaildropTarget": 9, "ExitNode": true, "ExitNodeOption": true},
    "k3": {"ID": "n3", "HostName": "phone", "DNSName": "phone.tail0.ts.net.", "OS": "iOS", "TailscaleIPs": ["100.64.0.3"], "Online": false, "Relay": "", "CurAddr": "", "LastSeen": "2026-11-10T08:00:00Z", "KeyExpiry": "2026-10-27T12:00:00Z", "Expired": true, "TaildropTarget": 5}
  }
}"#;

#[test]
fn status_lists_this_mac_first_then_peers_by_name() {
    let s = parse_status(STATUS).unwrap();
    assert!(s.running);
    let names: Vec<_> = s.peers.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["demo-mac", "lab", "phone"]);
    assert!(s.peers[0].this_mac);
    assert_eq!(s.peers[1].ip, "100.64.0.2");
    assert_eq!(s.peers[1].dns, "lab.tail0.ts.net");
    assert_eq!(s.peers[1].relay, "fra");
    assert_eq!(s.peers[0].last_seen, None);
    assert_eq!(s.peers[2].last_seen, Some(1_794_297_600));
    assert!(s.peers[2].expired);
    assert_eq!(s.peers[0].key_expiry, Some(1_799_539_200));
    assert_eq!(s.peers[1].key_expiry, None);
}

#[test]
fn a_stopped_tailscale_says_so() {
    let s = parse_status(r#"{"BackendState": "Stopped", "Self": null, "Peer": null}"#).unwrap();
    assert!(!s.running);
    assert!(s.peers.is_empty());
    assert!(parse_status("not json").is_err());
}

#[test]
fn pings_show_time_and_route() {
    assert_eq!(parse_ping("pong from lab (100.64.0.2) via DERP(fra) in 30ms\n"), Some(Ping { ms: 30.0, via: "DERP fra".into() }));
    assert_eq!(parse_ping("pong from lab (100.64.0.2) via 192.168.1.5:41641 in 3.5ms\n"), Some(Ping { ms: 3.5, via: "direct".into() }));
    assert_eq!(parse_ping("2026/11/18 direct connection not established\npong from lab (x) via DERP(fra) in 32ms"), Some(Ping { ms: 32.0, via: "DERP fra".into() }));
    assert_eq!(parse_ping("timeout waiting for ping reply\n"), None);
}

#[test]
fn only_computers_alert_when_they_come_and_go() {
    let s = parse_status(STATUS).unwrap();
    let mut seen = HashMap::new();
    assert!(changes(&mut seen, &s.peers).is_empty());
    let mut later = s.peers.clone();
    later[1].online = false;
    later[2].online = true;
    assert_eq!(changes(&mut seen, &later), vec![("lab".to_string(), false)]);
    later[1].online = true;
    assert_eq!(changes(&mut seen, &later), vec![("lab".to_string(), true)]);
}

#[test]
fn keys_that_expire_within_a_week_are_named() {
    let s = parse_status(STATUS).unwrap();
    let now = 1_799_539_200 - 3 * 86_400;
    assert_eq!(expiring(&s.peers, now), vec![("demo-mac".to_string(), 3), ("phone".to_string(), 0)]);
    assert_eq!(expiring(&s.peers, 1_799_539_200 - 30 * 86_400), vec![("phone".to_string(), 0)]);
}

#[test]
fn ssh_targets_are_plain() {
    assert!(valid_target("demo-dev@lab.tail0.ts.net"));
    assert!(valid_target("100.64.0.2"));
    assert!(!valid_target("-oProxyCommand=x"));
    assert!(!valid_target("a b"));
    assert!(!valid_target(""));
}


#[test]
fn the_exit_node_in_use_is_marked() {
    let s = parse_status(STATUS).unwrap();
    assert!(s.peers[1].exit_node);
    assert!(s.peers[1].exit_option);
    assert!(!s.peers[2].exit_node);
    assert!(!s.peers[2].exit_option);
}

#[test]
fn traffic_counts_come_with_each_device() {
    let json = r#"{"BackendState":"Running","Self":{"ID":"n1","HostName":"mac","Relay":"ams"},"Peer":{"k":{"ID":"n2","HostName":"vps","RxBytes":127594732,"TxBytes":24739804,"ExitNodeOption":true}}}"#;
    let s = parse_status(json).unwrap();
    assert_eq!((s.peers[1].rx_bytes, s.peers[1].tx_bytes), (127_594_732, 24_739_804));
    assert_eq!(s.peers[0].relay, "ams");
}

#[test]
fn the_exit_node_is_set_by_address_or_cleared() {
    assert_eq!(exit_node_args(Some("100.64.0.4")).unwrap(), ["set", "--exit-node=100.64.0.4"]);
    assert_eq!(exit_node_args(None).unwrap(), ["set", "--exit-node="]);
    assert!(exit_node_args(Some("--accept-routes")).is_err());
    assert!(exit_node_args(Some("a b")).is_err());
}
