use binder_core::Error;
use binder_core::remote::FakeRemote;
use binder_core::server::*;

const FULL: &str = include_str!("fixtures/server/full.txt");

fn section(name: &str) -> &'static str {
    let start = FULL.find(&format!("--binder:{name}--\n")).unwrap() + name.len() + 11;
    let rest = &FULL[start..];
    &rest[..rest.find("--binder:").unwrap_or(rest.len())]
}

#[test]
fn parses_docker_json_lines() {
    let c = parse_docker(section("docker"));
    assert_eq!(c.len(), 2);
    assert_eq!(
        c[0],
        Container { name: "jellyfin".into(), image: "jellyfin/jellyfin:latest".into(), state: "running".into(), status: "Up 3 days".into() }
    );
    assert_eq!(c[1].state, "exited");
}

#[test]
fn parses_running_services() {
    let s = parse_services(section("services"));
    assert_eq!(s.len(), 3);
    assert_eq!(s[2], Service { name: "ssh".into(), description: "OpenBSD Secure Shell server".into() });
}

#[test]
fn parses_load_mem_disk_temp() {
    assert_eq!(parse_loadavg(section("load")), [0.42, 0.37, 0.30]);
    assert_eq!(parse_meminfo(section("mem")), (8245760 * 1024, 4122880 * 1024));
    assert_eq!(parse_df(section("disk")), (62000000000, 50840000000));
    assert_eq!(parse_temp(section("temp")), Some(48.312));
}

#[test]
fn fetch_splits_sections_from_one_call() {
    let r = FakeRemote::new(vec![Ok(FULL.to_string())]);
    let s = fetch(&r, 1_700_000_000, None).unwrap();
    assert_eq!(r.seen.borrow().len(), 1);
    assert_eq!((s.containers.len(), s.services.len(), s.at), (2, 3, 1_700_000_000));
    assert_eq!(s.disk_used, 50840000000);
    assert_eq!(s.temp_c, Some(48.312));
    assert_eq!(s.cores, 4);
}

#[test]
fn parses_cores() {
    assert_eq!(parse_cores("4\n"), 4);
    assert_eq!(parse_cores(""), 0);
    assert_eq!(parse_cores("x"), 0);
}

#[test]
fn missing_docker_is_empty_not_error() {
    let out = "--binder:docker--\n--binder:services--\n--binder:load--\n0.1 0.2 0.3 1/2 3\n--binder:mem--\n--binder:disk--\n--binder:temp--\n";
    let s = fetch(&FakeRemote::new(vec![Ok(out.into())]), 0, None).unwrap();
    assert!(s.containers.is_empty() && s.services.is_empty());
    assert_eq!((s.mem_total, s.disk_size, s.temp_c), (0, 0, None));
    assert_eq!(s.load, [0.1, 0.2, 0.3]);
}

#[test]
fn unreachable_is_remote_error() {
    let r = FakeRemote::new(vec![Err(Error::Remote("Can't reach lab".into()))]);
    assert!(matches!(fetch(&r, 0, None), Err(Error::Remote(_))));
}

const PMIC: &str = include_str!("fixtures/server/pmic.txt");

#[test]
fn power_sums_volts_times_amps() {
    let w = parse_power(PMIC).unwrap();
    assert!((w - 4.50).abs() < 0.01, "{w}");
}

#[test]
fn power_is_none_without_pairs() {
    assert_eq!(parse_power(""), None);
    assert_eq!(parse_power("garbage\nEXT5V_V volt(24)=5.16V\n"), None);
}

#[test]
fn status_reads_power_and_survives_without_it() {
    let s = fetch(&FakeRemote::new(vec![Ok(FULL.into())]), 1, None).unwrap();
    assert!((s.power_w.unwrap() - 4.50).abs() < 0.01);
    let without = FULL[..FULL.find("--binder:power--").unwrap()].to_string();
    assert_eq!(fetch(&FakeRemote::new(vec![Ok(without)]), 1, None).unwrap().power_w, None);
}

const SD: &str = "/srv/cloud/SD";

fn with_sd(df: &str) -> String {
    format!("{FULL}--binder:sd--\n{df}")
}

#[test]
fn sd_is_read_when_it_is_its_own_mount() {
    let r = FakeRemote::new(vec![Ok(with_sd("Mounted on       1B-blocks         Used\n/srv/cloud/SD 127000000000 31000000000\n"))]);
    let s = fetch(&r, 1, Some(SD)).unwrap();
    assert_eq!((s.sd_size, s.sd_used), (Some(127_000_000_000), Some(31_000_000_000)));
    assert!(r.seen.borrow()[0].0.contains(&binder_core::remote::quote(SD)));
}

#[test]
fn sd_not_a_mount_is_left_out() {
    let r = FakeRemote::new(vec![Ok(with_sd("Mounted on 1B-blocks Used\n/ 1000000000000 400000000000\n"))]);
    let s = fetch(&r, 1, Some(SD)).unwrap();
    assert_eq!((s.sd_size, s.sd_used), (None, None));
    let none = fetch(&FakeRemote::new(vec![Ok(FULL.into())]), 1, None).unwrap();
    assert_eq!(none.sd_size, None);
}

#[test]
fn processes_parse_names_with_spaces() {
    let r = FakeRemote::new(vec![Ok(format!("--binder:ps--\n{}", include_str!("fixtures/server/ps-cpu.txt")))]);
    let p = processes(&r, false).unwrap();
    assert_eq!(p.len(), 3);
    assert_eq!((p[0].name.as_str(), p[0].cpu, p[0].mem, p[0].rss_mb), ("ollama", 41.2, 12.5, 1024));
    assert_eq!((p[1].name.as_str(), p[1].rss_mb), ("kworker/u8:2 something", 1));
    assert!(r.seen.borrow()[0].0.contains("--sort=-pcpu"));
}

#[test]
fn processes_ask_ps_by_memory() {
    let r = FakeRemote::new(vec![Ok("--binder:ps--\n".into())]);
    assert!(processes(&r, true).unwrap().is_empty());
    assert!(r.seen.borrow()[0].0.contains("--sort=-rss"));
}

#[test]
fn folders_keep_the_deepest_largest_first() {
    let f = parse_folders(include_str!("fixtures/server/du.txt"), &["/", SD]);
    let got: Vec<(&str, u64)> = f.iter().map(|f| (f.path.as_str(), f.bytes)).collect();
    assert_eq!(
        got,
        vec![
            ("/srv/cloud", 590_000_000_000),
            ("/home/demo", 150_000_000_000),
            ("/srv/cloud/SD/photos", 70_000_000_000),
            ("/var/lib", 30_000_000_000),
            ("/srv/cloud/SD/films", 20_000_000_000),
            ("/var/empty", 5000),
        ]
    );
}

#[test]
fn folders_quote_their_roots() {
    let r = FakeRemote::new(vec![Ok("--binder:du--\n".into())]);
    folders(&r, &["/", "/srv/my disk"]).unwrap();
    let script = r.seen.borrow()[0].0.clone();
    assert!(script.contains("'/srv/my disk'") && script.contains("du -x -B1 -d2"), "{script}");
}

#[test]
fn a_failed_docker_read_is_marked() {
    let s = fetch(&FakeRemote::new(vec![Ok("--binder:docker--\n--binder:docker-failed--\n--binder:load--\n0.1 0.2 0.3 1/2 3\n".into())]), 0, None).unwrap();
    assert!(s.docker_failed);
    assert!(s.containers.is_empty());
    let ok = fetch(&FakeRemote::new(vec![Ok("--binder:docker--\n--binder:load--\n0.1 0.2 0.3 1/2 3\n".into())]), 0, None).unwrap();
    assert!(!ok.docker_failed, "no containers at all is not a failure");
}

#[test]
fn the_docker_read_says_when_it_fails() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    fetch(&r, 0, None).unwrap();
    assert!(r.seen.borrow()[0].0.contains("|| echo --binder:docker-failed--"));
}

#[test]
fn uptime_gives_the_boot_time() {
    let out = "--binder:docker--\n--binder:uptime--\n3600.42 7000.00\n";
    let s = fetch(&FakeRemote::new(vec![Ok(out.into())]), 10_000, None).unwrap();
    assert_eq!(s.boot_at, Some(6400));
    let s = fetch(&FakeRemote::new(vec![Ok("--binder:docker--\n".into())]), 10_000, None).unwrap();
    assert_eq!(s.boot_at, None);
}

#[test]
fn container_times_parse_and_zero_times_are_none() {
    let out = "--binder:inspect--\n/web|running|2026-11-15T02:20:00.123Z|2026-11-15T02:14:00Z|2\n/db|created|0001-01-01T00:00:00Z|0001-01-01T00:00:00Z|0\n";
    let r = FakeRemote::new(vec![Ok(out.into())]);
    let t = binder_core::server::container_times(&r).unwrap();
    assert_eq!(t.len(), 2);
    assert_eq!((t[0].name.as_str(), t[0].state.as_str(), t[0].restarts), ("web", "running", 2));
    assert_eq!(t[0].started_at, Some(1_794_709_200));
    assert_eq!(t[0].finished_at, Some(1_794_708_840));
    assert_eq!((t[1].started_at, t[1].finished_at), (None, None));
}

#[test]
fn names_reach_inspect_through_docker_ps_only() {
    let r = FakeRemote::new(vec![Ok(String::new())]);
    binder_core::server::container_times(&r).unwrap();
    let script = r.seen.borrow()[0].0.clone();
    assert!(script.contains("docker inspect") && script.contains("$(docker ps -aq)"));
}

#[test]
fn a_name_reaches_the_shell_quoted() {
    let name = "web'; rm -rf ~";
    let r = FakeRemote::new(vec![Ok("--binder:logs--\nline one\nline two\n".into()), Ok(String::new())]);
    let log = binder_core::server::logs(&r, name).unwrap();
    assert_eq!(log, "line one\nline two");
    binder_core::server::restart(&r, name).unwrap();
    let seen = r.seen.borrow();
    let q = binder_core::remote::quote(name);
    assert!(seen[0].0.contains(&format!("docker logs --tail 200 {q} 2>&1")), "{}", seen[0].0);
    assert!(seen[1].0.contains(&format!("docker restart {q}")), "{}", seen[1].0);
}

#[test]
fn restart_logs_it_as_yours() {
    let db = std::sync::Mutex::new(binder_core::Db::in_memory().unwrap());
    let r = FakeRemote::new(vec![Ok(String::new())]);
    binder_core::server::restart_and_log(&db, &r, "web", 500).unwrap();
    let events = db.lock().unwrap().events_between(0, 1000).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].kind, events[0].subject.as_str(), events[0].at), (binder_core::events::EventKind::OwnerRestart, "web", 500));
}

#[test]
fn a_failed_restart_leaves_no_owner_restart() {
    let db = std::sync::Mutex::new(binder_core::Db::in_memory().unwrap());
    let r = FakeRemote::new(vec![Err(Error::Remote("No such container".into()))]);
    assert!(binder_core::server::restart_and_log(&db, &r, "web", 500).is_err());
    // Nothing was restarted, so the alerts for web stay on.
    assert!(db.lock().unwrap().events_between(0, 1000).unwrap().is_empty());
}

#[test]
fn the_servers_own_name_comes_back() {
    let out = "--binder:load--\n0.1 0.2 0.3 1/2 3\n--binder:host--\nhomelab\n";
    let s = fetch(&FakeRemote::new(vec![Ok(out.into())]), 0, None).unwrap();
    assert_eq!(s.hostname, "homelab");
    let r = FakeRemote::new(vec![Ok(String::new())]);
    fetch(&r, 0, None).unwrap();
    assert!(r.seen.borrow()[0].0.contains("echo --binder:host--; hostname"));
}
