use binder_core::usage::*;

const LINE_A: &str = r#"{"parentUuid":"x","message":{"id":"msg_01","role":"assistant","usage":{"input_tokens":10,"cache_creation_input_tokens":200,"cache_read_input_tokens":5000,"output_tokens":30}},"timestamp":"2026-11-17T09:00:00.000Z"}"#;
const LINE_A_AGAIN: &str = r#"{"message":{"id":"msg_01","usage":{"input_tokens":10,"cache_creation_input_tokens":200,"cache_read_input_tokens":5000,"output_tokens":30}},"timestamp":"2026-11-17T09:00:01.000Z"}"#;
const LINE_B: &str = r#"{"message":{"id":"msg_02","usage":{"input_tokens":1,"output_tokens":2}},"timestamp":"2026-11-12T09:00:00Z"}"#;
const USER: &str = r#"{"type":"user","message":{"content":"how many input_tokens?"},"timestamp":"2026-11-17T08:00:00Z"}"#;

fn at(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s).unwrap().timestamp()
}

#[test]
fn the_script_reads_transcripts() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/projects/-home-demo-dev-code");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.jsonl"), format!("{LINE_A}\n{USER}\n{LINE_A_AGAIN}\n")).unwrap();
    std::fs::write(dir.join("b.jsonl"), format!("{LINE_B}\n")).unwrap();
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(SCRIPT).env("HOME", home.path()).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut uses = parse(&String::from_utf8_lossy(&out.stdout));
    uses.sort_by_key(|u| u.at);
    assert_eq!(uses.len(), 3);
    assert_eq!(uses[0].tokens, 3);
    assert_eq!(uses[1].id, "msg_01");
    assert_eq!(uses[1].tokens, 240);
    assert_eq!(uses[1].at, at("2026-11-17T09:00:00Z"));
}

#[test]
fn a_missing_folder_reads_nothing() {
    let home = tempfile::tempdir().unwrap();
    let out = std::process::Command::new("/bin/sh").arg("-c").arg(SCRIPT).env("HOME", home.path()).output().unwrap();
    assert!(out.status.success());
    assert!(parse(&String::from_utf8_lossy(&out.stdout)).is_empty());
}

#[test]
fn summary_counts_each_message_once() {
    let u = |at_: &str, id: &str, file: &str, tokens| Use { at: at(at_), id: id.into(), file: file.into(), tokens };
    let uses = vec![
        u("2026-11-17T09:00:00Z", "msg_01", "a", 240),
        u("2026-11-17T09:00:01Z", "msg_01", "a", 240),
        u("2026-11-17T01:00:00Z", "msg_03", "c", 100),
        u("2026-11-16T22:00:00Z", "msg_04", "c", 50),
        u("2026-11-12T09:00:00Z", "msg_02", "b", 3),
        u("2026-11-01T09:00:00Z", "msg_05", "d", 9999),
    ];
    let now = at("2026-11-17T10:00:00Z");
    let midnight = at("2026-11-17T00:00:00Z");
    let s = summary(&uses, now, midnight);
    assert_eq!(s.five_hours, 240);
    assert_eq!(s.today, 340);
    assert_eq!(s.week, 393);
    assert_eq!(s.sessions_today, 2);
}

#[test]
fn summaries_add_up() {
    let a = Usage { five_hours: 1, today: 2, week: 3, sessions_today: 1, ..Default::default() };
    let b = Usage { five_hours: 10, today: 20, week: 30, sessions_today: 2, ..Default::default() };
    assert_eq!(a + b, Usage { five_hours: 11, today: 22, week: 33, sessions_today: 3, ..Default::default() });
}

#[test]
fn bad_lines_are_skipped() {
    assert!(parse("nonsense\n|||\n2026-13-99T00:00:00Z|m|f|1|1|1|1\n").is_empty());
}

#[test]
fn the_mac_reader_matches_the_script_and_caches() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/projects/p");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.jsonl"), format!("{LINE_A}\n{USER}\n{LINE_A_AGAIN}\n{LINE_B}\n")).unwrap();
    std::fs::write(dir.join("notes.txt"), format!("{LINE_A}\n")).unwrap();
    let mut cache = Cache::default();
    let mut uses = cache.read(&home.path().join(".claude/projects"));
    uses.sort_by_key(|u| u.at);
    assert_eq!(uses.iter().map(|u| u.tokens).collect::<Vec<_>>(), vec![3, 240, 240]);
    assert_eq!(uses[1].id, "msg_01");
    // Unchanged files are read from the cache; a grown file is read again.
    assert_eq!(cache.read(&home.path().join(".claude/projects")).len(), 3);
    std::fs::write(dir.join("a.jsonl"), format!("{LINE_A}\n{LINE_B}\n{}\n", LINE_B.replace("msg_02", "msg_09"))).unwrap();
    assert_eq!(cache.read(&home.path().join(".claude/projects")).len(), 3);
    assert!(cache.read(&home.path().join("nowhere")).is_empty());
}

#[test]
fn days_split_the_week_by_local_midnight() {
    let midnight = 1_000_000;
    let at = |t: i64, id: &str| binder_core::usage::Use { at: t, id: id.into(), file: "f".into(), tokens: 10 };
    let uses = vec![at(midnight + 5, "a"), at(midnight - 5, "b"), at(midnight - 86_400 - 1, "c"), at(midnight - 6 * 86_400 + 1, "d"), at(midnight - 7 * 86_400, "e")];
    let u = binder_core::usage::summary(&uses, midnight + 10, midnight);
    assert_eq!(u.days, [10, 0, 0, 0, 10, 10, 10]);
    assert_eq!((u + u).days[6], 20);
}
