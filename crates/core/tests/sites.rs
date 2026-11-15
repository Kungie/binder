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

#[test]
fn a_snapshot_resolves_against_the_site_and_has_no_scripts() {
    let html = "<!doctype html><html><HEAD lang=en><title>Shop</title><script src=\"/app.js\"></script></head><body><img src=\"/logo.png\"><SCRIPT>alert(1)</SCRIPT>Hi</body></html>";
    let doc = sites::snapshot(html, "https://shop.example.com/");
    assert!(doc.contains("<HEAD lang=en><base href=\"https://shop.example.com/\">"), "{doc}");
    assert!(!doc.to_lowercase().contains("<script"), "{doc}");
    assert!(doc.contains("<img src=\"/logo.png\">Hi"));
}

#[test]
fn a_snapshot_without_a_head_still_gets_its_base() {
    let doc = sites::snapshot("<p>Hi</p>", "https://a.example.com/x\"y");
    assert!(doc.starts_with("<base href=\"https://a.example.com/x&quot;y\">"), "{doc}");
}

#[test]
fn a_meta_refresh_is_dropped_from_a_snapshot() {
    let doc = sites::snapshot("<head><meta http-equiv=\"refresh\" content=\"0;url=/x\"><meta charset=utf-8></head>", "https://a.example.com/");
    assert!(!doc.contains("refresh"), "{doc}");
    assert!(doc.contains("<meta charset=utf-8>"));
}

const FOUND: &str = "--binder:sites--
blog|/home/demo-dev/sites/blog|blog.example.com|100
shop|/home/demo-dev/sites/shop||200
--binder:ps--
blog-site-1|running|/home/demo-dev/sites/blog|blog|nginx
blog-cloudflared-1|running|/home/demo-dev/sites/blog|blog|cloudflare/cloudflared:latest
shop-web-1|running|/home/demo-dev/sites/shop|shop|node
shop-tunnel-1|running|/home/demo-dev/shop-old|shop|cloudflare/cloudflared:2026.10.4
bot-app-1|running|/home/demo-dev/apps/bot|bot|python
bot-cloudflared-1|exited|/home/demo-dev/apps/bot|bot|cloudflare/cloudflared:latest
grafana|running|/home/demo-dev/apps/grafana|grafana|grafana
--binder:tunnels--
blog|/home/demo-dev/sites/blog|logs.example.com|100
shop|/home/demo-dev/sites/shop|shop.example.com|200
bot|/home/demo-dev/apps/bot|bot.example.com|300
--binder:stats--
bot-app-1|1.00%|10MiB / 7.75GiB
";

fn found() -> Vec<sites::Site> {
    sites::read(&FakeRemote::new(vec![Ok(FOUND.into())]), &BTreeMap::new()).unwrap()
}

#[test]
fn a_site_hosted_outside_the_sites_folder_is_found_by_its_tunnel() {
    let s = found();
    assert_eq!(s.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["blog", "shop", "bot"]);
    let bot = &s[2];
    assert_eq!(bot.address.as_deref(), Some("https://bot.example.com"));
    assert_eq!(bot.disk_bytes, 300);
    assert_eq!(bot.containers.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["bot-app-1", "bot-cloudflared-1"]);
    assert_eq!(bot.mem_bytes, 10 * 1024 * 1024);
}

#[test]
fn a_tunnel_managed_on_cloudflare_gives_its_hostname() {
    let s = found();
    // The local tunnel config wins; a tunnel set up on Cloudflare's side gives the address from its log.
    assert_eq!(s[0].address.as_deref(), Some("https://blog.example.com"));
    assert_eq!(s[1].address.as_deref(), Some("https://shop.example.com"));
}

#[test]
fn a_container_joins_its_site_by_compose_project_too() {
    let s = found();
    assert_eq!(s[1].containers.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["shop-web-1", "shop-tunnel-1"]);
}

#[test]
fn tunnel_hostnames_come_only_from_log_lines_that_name_them() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    sites::read(&r, &BTreeMap::new()).unwrap();
    let script = r.seen.borrow()[0].0.clone();
    assert!(script.contains("--binder:tunnels--") && script.contains("docker logs") && script.contains("cloudflared"));
    // Only the hostname itself leaves the server.
    assert!(script.contains("grep -oE"));
}

#[test]
fn a_snapshot_holds_still() {
    let doc = sites::snapshot("<html><head><title>x</title></head><body><p>Hi</p></body></html>", "https://a.example.com/");
    // Nothing moves in a preview, so a page of cards costs nothing once drawn.
    assert!(doc.contains("animation: none !important") && doc.contains("transition: none !important"));
    assert!(doc.find("animation: none").unwrap() < doc.find("<title>").unwrap());
}
