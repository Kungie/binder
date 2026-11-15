use binder_core::remote::FakeRemote;
use binder_core::sites::{self, SiteContainer};
use std::collections::BTreeMap;

const OUT: &str = "--binder:sites--
blog|/home/demo-dev/sites/blog|blog.example.com|12582912
shop|/home/demo-dev/sites/shop||4096
--binder:ps--
blog-site-1|running|/home/demo-dev/sites/blog
blog-cloudflared-1|running|/home/demo-dev/sites/blog
shop-site-1|exited|/home/demo-dev/sites/shop
grafana|running|/home/demo-dev/apps/grafana
--binder:stats--
blog-site-1|0.30%|30MiB / 7.75GiB
blog-cloudflared-1|0.10%|8.5MiB / 7.75GiB
grafana|2.00%|120MiB / 7.75GiB
";

fn read(saved: &BTreeMap<String, String>) -> Vec<sites::Site> {
    sites::read(&FakeRemote::new(vec![Ok(OUT.into())]), saved).unwrap()
}

#[test]
fn sites_are_folders_with_a_compose_file() {
    let s = read(&BTreeMap::new());
    assert_eq!(s.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["blog", "shop"]);
}

#[test]
fn the_address_comes_from_hostname_lines_only() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    sites::read(&r, &BTreeMap::new()).unwrap();
    let script = r.seen.borrow()[0].0.clone();
    assert!(script.contains("hostname:") && script.contains("cloudflared/config.yml") && script.contains("docker-compose.yml"));
    for never in ["cat ", ".env", "credentials", "token", "TUNNEL"] {
        assert!(!script.contains(never), "the script mentions {never}");
    }
    let s = read(&BTreeMap::new());
    assert_eq!(s[0].address.as_deref(), Some("https://blog.example.com"));
    assert_eq!(s[1].address, None);
}

#[test]
fn containers_join_their_site_by_working_dir() {
    let s = read(&BTreeMap::new());
    assert_eq!(
        s[0].containers,
        vec![SiteContainer { name: "blog-site-1".into(), state: "running".into() }, SiteContainer { name: "blog-cloudflared-1".into(), state: "running".into() }]
    );
    assert_eq!(s[1].containers, vec![SiteContainer { name: "shop-site-1".into(), state: "exited".into() }]);
}

#[test]
fn stats_add_up_per_site() {
    let s = read(&BTreeMap::new());
    assert!((s[0].cpu - 0.4).abs() < 1e-4, "{}", s[0].cpu);
    assert_eq!(s[0].mem_bytes, 30 * 1024 * 1024 + (8.5 * 1024.0 * 1024.0) as u64);
    assert_eq!(s[0].disk_bytes, 12_582_912);
    assert_eq!((s[1].cpu, s[1].mem_bytes, s[1].disk_bytes), (0.0, 0, 4096));
}

#[test]
fn a_saved_address_wins() {
    let saved = BTreeMap::from([("shop".to_string(), "https://shop.example.com".to_string()), ("blog".to_string(), "https://www.example.com".to_string())]);
    let s = read(&saved);
    assert_eq!(s[0].address.as_deref(), Some("https://www.example.com"));
    assert_eq!(s[1].address.as_deref(), Some("https://shop.example.com"));
}

#[test]
fn parse_check_reads_status_and_frame_rules() {
    let ok = sites::parse_check("HTTP/2 301\r\nlocation: https://www.example.com/\r\n\r\nHTTP/2 200\r\ncontent-type: text/html\r\n\r\n");
    assert_eq!((ok.status, ok.frameable), (Some(200), true));
    let deny = sites::parse_check("HTTP/2 200\r\nX-Frame-Options: DENY\r\n\r\n");
    assert_eq!((deny.status, deny.frameable), (Some(200), false));
    let csp = sites::parse_check("HTTP/1.1 200 OK\r\ncontent-security-policy: default-src 'self'; frame-ancestors 'self'\r\n\r\n");
    assert!(!csp.frameable);
    let star = sites::parse_check("HTTP/2 502\r\ncontent-security-policy: frame-ancestors *\r\n\r\n");
    assert_eq!((star.status, star.frameable), (Some(502), true));
}

#[test]
fn memory_sizes_parse() {
    assert_eq!(sites::bytes("38.2MiB"), (38.2 * 1024.0 * 1024.0) as u64);
    assert_eq!(sites::bytes("1.5GiB"), (1.5 * 1024.0 * 1024.0 * 1024.0) as u64);
    assert_eq!(sites::bytes("512KiB"), 512 * 1024);
    assert_eq!(sites::bytes("12MB"), 12_000_000);
    assert_eq!(sites::bytes("900B"), 900);
}
