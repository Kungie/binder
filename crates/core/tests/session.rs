use binder_core::model::*;
use binder_core::notes;
use binder_core::redact::redact;
use binder_core::session::*;
use binder_core::{Db, Result};
use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("fixtures/transcripts/a.jsonl");

struct Fake {
    replies: RefCell<Vec<Vec<String>>>,
    seen: RefCell<Vec<(Vec<String>, String)>>,
}

impl Fake {
    fn new(replies: Vec<Vec<&str>>) -> Fake {
        Fake { replies: RefCell::new(replies.into_iter().map(|r| r.into_iter().map(String::from).collect()).collect()), seen: RefCell::new(vec![]) }
    }
}

impl Summarizer for Fake {
    fn lines(&self, done: &[String], talk: &str) -> Result<Vec<String>> {
        self.seen.borrow_mut().push((done.to_vec(), talk.to_string()));
        let mut r = self.replies.borrow_mut();
        Ok(if r.is_empty() { vec![] } else { r.remove(0) })
    }
}

struct World {
    db: Db,
    dir: tempfile::TempDir,
    project: Project,
}

impl World {
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("vault")).unwrap();
        std::fs::create_dir_all(dir.path().join("code/orbit/src")).unwrap();
        let mut db = Db::in_memory().unwrap();
        let repo = dir.path().join("code/orbit").to_string_lossy().into_owned();
        let project = db.create_project(NewProject { name: "orbit".into(), repo: Some(repo), ..Default::default() }, 1).unwrap();
        World { db, dir, project }
    }
    fn vault(&self) -> PathBuf {
        self.dir.path().join("vault")
    }
    fn transcript(&self, name: &str, text: &str) -> PathBuf {
        let p = self.dir.path().join(name);
        std::fs::OpenOptions::new().create(true).append(true).open(&p).unwrap().write_all(text.as_bytes()).unwrap();
        p
    }
    fn hook(&mut self, session: &str, path: &Path, event: &str) -> HookAction {
        let input = HookInput {
            session_id: session.into(),
            transcript_path: path.to_string_lossy().into_owned(),
            cwd: self.dir.path().join("code/orbit/src").to_string_lossy().into_owned(),
            hook_event_name: event.into(),
        };
        on_hook(&mut self.db, &input, 100).unwrap()
    }
    fn note(&mut self) -> PathBuf {
        PathBuf::from(self.db.get_project(self.project.id).unwrap().note.unwrap())
    }
}

fn talk_line(role: &str, text: &str) -> String {
    let content = if role == "user" { serde_json::json!(text) } else { serde_json::json!([{ "type": "text", "text": text }]) };
    format!("{}\n", serde_json::json!({ "type": role, "message": { "role": role, "content": content } }))
}

#[test]
fn reads_only_text_after_offset() {
    let w = World::new();
    let p = w.transcript("a.jsonl", FIXTURE);
    let (talk, end) = new_talk(&p, 0).unwrap();
    assert_eq!(
        talk,
        "User: Should the sync run in a worker or on the main thread?\nAssistant: A worker. The main thread must stay free for input.\nAssistant: Moved sync into a worker.\n"
    );
    assert_eq!(end, FIXTURE.len() as u64);
    assert_eq!(new_talk(&p, end).unwrap(), (String::new(), end));
    w.transcript("a.jsonl", &talk_line("user", "Next step?"));
    assert_eq!(new_talk(&p, end).unwrap().0, "User: Next step?\n");
    // A line still being written isn't read yet.
    w.transcript("a.jsonl", "{\"type\":\"user\",\"mess");
    let (_, end2) = new_talk(&p, end).unwrap();
    assert_eq!(new_talk(&p, end2).unwrap().0, "");
}

#[test]
fn offset_past_end_restarts() {
    let w = World::new();
    let p = w.transcript("a.jsonl", FIXTURE);
    let (talk, end) = new_talk(&p, 99_999).unwrap();
    assert!(talk.starts_with("User: Should the sync"));
    assert_eq!(end, FIXTURE.len() as u64);
}

#[test]
fn stop_waits_for_4000_chars() {
    let mut w = World::new();
    let p = w.transcript("s.jsonl", &talk_line("user", &"a".repeat(1000)));
    assert_eq!(w.hook("s1", &p, "Stop"), HookAction::Nothing);
    w.transcript("s.jsonl", &talk_line("assistant", &"b".repeat(3100)));
    assert_eq!(w.hook("s1", &p, "Stop"), HookAction::Summarize { session_id: "s1".into() });
}

#[test]
fn precompact_summarizes_any_talk() {
    let mut w = World::new();
    let p = w.transcript("s.jsonl", &talk_line("user", "Use SQLite."));
    assert_eq!(w.hook("s1", &p, "PreCompact"), HookAction::Summarize { session_id: "s1".into() });
    let q = w.transcript("t.jsonl", &talk_line("user", "Ship it."));
    assert_eq!(w.hook("s2", &q, "SessionEnd"), HookAction::Summarize { session_id: "s2".into() });
    let empty = w.transcript("e.jsonl", "");
    assert_eq!(w.hook("s3", &empty, "SessionEnd"), HookAction::Nothing);
}

#[test]
fn two_sessions_keep_own_cursors() {
    let mut w = World::new();
    let a = w.transcript("a.jsonl", &talk_line("user", "Decision A"));
    let b = w.transcript("b.jsonl", &talk_line("user", "Decision B"));
    w.hook("sa", &a, "SessionEnd");
    w.hook("sb", &b, "SessionEnd");
    let s = Fake::new(vec![vec![], vec![], vec![]]);
    let vault = w.vault();
    summarize(&mut w.db, &vault, &s, "sa", "2026-11-13", "14:32").unwrap();
    summarize(&mut w.db, &vault, &s, "sb", "2026-11-13", "14:32").unwrap();
    summarize(&mut w.db, &vault, &s, "sa", "2026-11-13", "14:32").unwrap();
    let seen = s.seen.borrow();
    assert_eq!(seen.len(), 2, "the third call had nothing new and didn't reach the model");
    assert_eq!(seen[0].1, "User: Decision A\n");
    assert_eq!(seen[1].1, "User: Decision B\n");
}

#[test]
fn summarize_appends_new_lines_and_skips_repeats() {
    let mut w = World::new();
    let vault = w.vault();
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["Sync runs in a worker."], vec!["sync runs in a worker", "Keep the main thread for input."]]);
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-13", "14:32").unwrap(), 1);
    w.transcript("a.jsonl", &talk_line("user", "And input?"));
    w.hook("s1", &p, "SessionEnd");
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-14", "14:32").unwrap(), 1);
    let note = w.note();
    let lines: Vec<String> = notes::read_history(&note).unwrap().into_iter().map(|l| l.text).collect();
    assert_eq!(lines, ["Keep the main thread for input.", "Sync runs in a worker."]);
    assert_eq!(s.seen.borrow()[1].0, ["Sync runs in a worker."], "the model saw the log");
}

#[test]
fn at_most_three_lines() {
    let mut w = World::new();
    let vault = w.vault();
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["One.", "Two.", "Three.", "Four."]]);
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-13", "14:32").unwrap(), 3);
}

#[test]
fn vault_missing_queues_then_flushes_once() {
    let mut w = World::new();
    let missing = w.dir.path().join("not-mounted");
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["Sync runs in a worker."]]);
    assert_eq!(summarize(&mut w.db, &missing, &s, "s1", "2026-11-13", "14:32").unwrap(), 1);
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 0);
    std::fs::create_dir_all(&missing).unwrap();
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 1);
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 0);
    let note = w.note();
    let done = notes::read_history(&note).unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!((done[0].date.as_str(), done[0].text.as_str()), ("2026-11-13", "Sync runs in a worker."));
}

#[test]
fn cwd_outside_projects_does_nothing() {
    let mut w = World::new();
    let p = w.transcript("a.jsonl", FIXTURE);
    let input = HookInput { session_id: "x".into(), transcript_path: p.to_string_lossy().into_owned(), cwd: "/tmp/elsewhere".into(), hook_event_name: "SessionEnd".into() };
    assert_eq!(on_hook(&mut w.db, &input, 1).unwrap(), HookAction::Nothing);
    // A sibling folder that only shares a name prefix isn't inside the repo.
    let sibling = w.dir.path().join("code/orbit-old").to_string_lossy().into_owned();
    let input = HookInput { cwd: sibling, ..input };
    assert_eq!(on_hook(&mut w.db, &input, 1).unwrap(), HookAction::Nothing);
    assert!(project_for(&w.db, &w.dir.path().join("code/orbit/src").to_string_lossy()).is_some());
}

#[test]
fn redacts_common_secrets() {
    let text = "key sk-ant-api03-AAAABBBBCCCCDDDDEEEE and ghp_0123456789abcdefghijABCDEFGHIJ\n\
                aws AKIAABCDEFGHIJKLMNOP\n\
                -----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----\n\
                password=hunter2 token: abc123 Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.e30.x\n\
                blob 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\n\
                path src/sync/worker.rs and a sentence.";
    let r = redact(text);
    for secret in ["AAAABBBB", "ghp_0123", "AKIAABCD", "b3BlbnNz", "hunter2", "abc123", "eyJhbGci", "9f86d081"] {
        assert!(!r.contains(secret), "{secret} leaked: {r}");
    }
    assert!(r.contains("path src/sync/worker.rs and a sentence."));
    assert!(r.contains("password="));
}

#[test]
fn parse_lines_handles_chatter_around_json() {
    assert_eq!(parse_lines("Sure. {\"lines\": [\"Use SQLite.\", \"Drop tmux.\"]} Hope that helps {x}"), ["Use SQLite.", "Drop tmux."]);
    assert_eq!(parse_lines("```json\n{\"lines\": []}\n```"), Vec::<String>::new());
    assert_eq!(parse_lines("{not json} then {\"lines\": [\"A {brace} inside.\"]}"), ["A {brace} inside."]);
    assert_eq!(parse_lines("nothing here"), Vec::<String>::new());
}

#[test]
fn concurrent_flushes_write_each_line_once() {
    let w = World::new();
    let path = w.dir.path().join("binder.db");
    let mut db = Db::open(&path).unwrap();
    let p = db.create_project(NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    for i in 0..40 {
        conn.execute("INSERT INTO pending_done (project_id, date, text) VALUES (?1, '2026-11-13', ?2)", rusqlite::params![p.id, format!("Line {i}.")]).unwrap();
    }
    let vault = w.vault();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (path, vault, barrier) = (path.clone(), vault.clone(), barrier.clone());
            std::thread::spawn(move || {
                let mut db = Db::open(&path).unwrap();
                barrier.wait();
                flush_pending(&mut db, &vault).unwrap()
            })
        })
        .collect();
    let written: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(written, 40);
    let note = PathBuf::from(db.get_project(p.id).unwrap().note.unwrap());
    let lines = notes::read_history(&note).unwrap();
    assert_eq!(lines.len(), 40, "each line exactly once");
}

#[test]
fn concurrent_summaries_read_the_talk_once() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Slow(std::sync::Arc<AtomicUsize>);
    impl Summarizer for Slow {
        fn lines(&self, _: &[String], _: &str) -> Result<Vec<String>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(50));
            Ok(vec![])
        }
    }
    let w = World::new();
    let path = w.dir.path().join("binder.db");
    let repo = w.dir.path().join("code/orbit").to_string_lossy().into_owned();
    let mut db = Db::open(&path).unwrap();
    db.create_project(NewProject { name: "orbit".into(), repo: Some(repo), ..Default::default() }, 1).unwrap();
    let t = w.transcript("a.jsonl", FIXTURE);
    let input = HookInput {
        session_id: "s1".into(),
        transcript_path: t.to_string_lossy().into_owned(),
        cwd: w.dir.path().join("code/orbit").to_string_lossy().into_owned(),
        hook_event_name: "SessionEnd".into(),
    };
    on_hook(&mut db, &input, 1).unwrap();
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    for _ in 0..20 {
        // Each round adds new talk and races two summaries for it.
        w.transcript("a.jsonl", &talk_line("user", "More."));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let before = calls.load(Ordering::SeqCst);
        let hs: Vec<_> = (0..2)
            .map(|_| {
                let (path, vault, barrier, calls) = (path.clone(), w.vault(), barrier.clone(), calls.clone());
                std::thread::spawn(move || {
                    let mut db = Db::open(&path).unwrap();
                    barrier.wait();
                    summarize(&mut db, &vault, &Slow(calls), "s1", "2026-11-13", "14:32").unwrap()
                })
            })
            .collect();
        hs.into_iter().for_each(|h| { h.join().unwrap(); });
        assert_eq!(calls.load(Ordering::SeqCst) - before, 1, "the same talk reached the model twice");
    }
}

#[test]
fn summary_writes_daily_file() {
    let mut w = World::new();
    let vault = w.vault();
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    assert_eq!(w.db.meta("last_done").unwrap(), None);
    let s = Fake::new(vec![vec!["Sync runs in a worker."]]);
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-13", "14:32").unwrap(), 1);
    let note = w.note();
    assert_eq!(note, vault.join("Projects/orbit/orbit.md"));
    let daily = std::fs::read_to_string(vault.join("Projects/orbit/Daily/2026-11-13.md")).unwrap();
    assert!(daily.ends_with("\n- 14:32 · Sync runs in a worker.\n"), "{daily}");
    assert!(!std::fs::read_to_string(&note).unwrap().contains("Sync runs"), "the note itself is left alone");
    let last: i64 = w.db.meta("last_done").unwrap().expect("last_done set").parse().unwrap();
    assert!(last > 1_700_000_000);
}

#[test]
fn pending_lines_land_in_their_own_day() {
    let mut w = World::new();
    let vault = w.dir.path().join("later");
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["Queued on the 12th."]]);
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-12", "23:10").unwrap(), 1);
    std::fs::create_dir_all(&vault).unwrap();
    // The flush runs on the 13th; the line still belongs to the 12th.
    assert_eq!(flush_pending(&mut w.db, &vault).unwrap(), 1);
    let note = w.note();
    assert_eq!(notes::read_daily(&note, "2026-11-12").unwrap(), vec![notes::DoneLine { date: "2026-11-12".into(), time: Some("23:10".into()), text: "Queued on the 12th.".into() }]);
    assert!(notes::read_daily(&note, "2026-11-13").unwrap().is_empty());
    assert!(w.db.meta("last_done").unwrap().is_some());
}

#[test]
fn stored_loose_note_moves_into_its_folder() {
    // The project was renamed after its note was made, so the note keeps its own name.
    let mut w = World::new();
    let vault = w.vault();
    let loose = vault.join("Projects/old orbit.md");
    std::fs::create_dir_all(loose.parent().unwrap()).unwrap();
    std::fs::write(&loose, "# orbit\n\n## Done\n- 2026-11-01 · old\n").unwrap();
    w.db.set_project_note(w.project.id, &loose.to_string_lossy()).unwrap();
    let path = note_for(&mut w.db, &vault, w.project.id, "2026-11-13").unwrap();
    assert_eq!(path, vault.join("Projects/old orbit/old orbit.md"));
    assert_eq!(w.note(), path, "the new path is stored");
    assert!(!vault.join("Projects/orbit").exists(), "no second note");
    assert!(!loose.exists());
    assert_eq!(notes::read_history(&path).unwrap()[0].text, "old");
}
