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
    let s = fetch(&r, 1_700_000_000).unwrap();
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
    let s = fetch(&FakeRemote::new(vec![Ok(out.into())]), 0).unwrap();
    assert!(s.containers.is_empty() && s.services.is_empty());
    assert_eq!((s.mem_total, s.disk_size, s.temp_c), (0, 0, None));
    assert_eq!(s.load, [0.1, 0.2, 0.3]);
}

#[test]
fn unreachable_is_remote_error() {
    let r = FakeRemote::new(vec![Err(Error::Remote("Can't reach lab".into()))]);
    assert!(matches!(fetch(&r, 0), Err(Error::Remote(_))));
}
