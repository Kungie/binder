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
    summarize(&mut w.db, &vault, &s, "sa", "2026-11-13").unwrap();
    summarize(&mut w.db, &vault, &s, "sb", "2026-11-13").unwrap();
    summarize(&mut w.db, &vault, &s, "sa", "2026-11-13").unwrap();
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
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-13").unwrap(), 1);
    w.transcript("a.jsonl", &talk_line("user", "And input?"));
    w.hook("s1", &p, "SessionEnd");
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-14").unwrap(), 1);
    let note = w.note();
    let lines: Vec<String> = notes::read_done(&note).unwrap().into_iter().map(|l| l.text).collect();
    assert_eq!(lines, ["Sync runs in a worker.", "Keep the main thread for input."]);
    assert_eq!(s.seen.borrow()[1].0, ["Sync runs in a worker."], "the model saw the log");
}

#[test]
fn at_most_three_lines() {
    let mut w = World::new();
    let vault = w.vault();
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["One.", "Two.", "Three.", "Four."]]);
    assert_eq!(summarize(&mut w.db, &vault, &s, "s1", "2026-11-13").unwrap(), 3);
}

#[test]
fn vault_missing_queues_then_flushes_once() {
    let mut w = World::new();
    let missing = w.dir.path().join("not-mounted");
    let p = w.transcript("a.jsonl", FIXTURE);
    w.hook("s1", &p, "SessionEnd");
    let s = Fake::new(vec![vec!["Sync runs in a worker."]]);
    assert_eq!(summarize(&mut w.db, &missing, &s, "s1", "2026-11-13").unwrap(), 1);
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 0);
    std::fs::create_dir_all(&missing).unwrap();
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 1);
    assert_eq!(flush_pending(&mut w.db, &missing).unwrap(), 0);
    let note = w.note();
    let done = notes::read_done(&note).unwrap();
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
