//! Done lines from Claude Code sessions. Hooks report a session's transcript; once enough new talk waits,
//! a small model picks out the key decisions and they're appended to the project's note.

use crate::model::Project;
use crate::notes::{self, one_line};
use crate::redact::redact;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// New talk that makes a `Stop` hook summarize.
pub const STOP_THRESHOLD: usize = 4_000;
/// The model sees at most this much of the newest talk.
pub const MAX_TALK: usize = 12_000;
pub const MAX_LINES: usize = 3;
/// Done lines the model sees, so it doesn't repeat them.
pub const LOG_LINES: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HookInput {
    pub session_id: String,
    pub transcript_path: String,
    pub cwd: String,
    pub hook_event_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookAction {
    Nothing,
    Summarize { session_id: String },
}

fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// The project whose repo folder holds `cwd`; the deepest one when repos nest.
pub fn project_for(db: &Db, cwd: &str) -> Option<Project> {
    let cwd = Path::new(cwd);
    db.list_projects()
        .ok()?
        .into_iter()
        .filter_map(|p| {
            let repo = expand_home(p.repo.as_deref()?);
            cwd.starts_with(&repo).then(|| (repo.components().count(), p))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, p)| p)
}

fn text_of(content: &serde_json::Value) -> Vec<String> {
    match content {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .filter_map(|b| b.get("text")?.as_str().map(String::from))
            .collect(),
        _ => vec![],
    }
}

/// The user and assistant text after `offset`, one `User: …` or `Assistant: …` line per block, and the offset
/// just past the last complete line read. Tool calls, tool results and thinking are skipped.
/// An offset past the end means the file was rewritten, so reading starts over.
pub fn new_talk(path: &Path, offset: u64) -> Result<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = if offset > len { 0 } else { offset };
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let complete = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let mut talk = String::new();
    for line in String::from_utf8_lossy(&bytes[..complete]).lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let who = match v.get("type").and_then(|t| t.as_str()) {
            Some("user") => "User",
            Some("assistant") => "Assistant",
            _ => continue,
        };
        let Some(content) = v.get("message").and_then(|m| m.get("content")) else { continue };
        for text in text_of(content) {
            let text = text.trim();
            if !text.is_empty() {
                talk.push_str(&format!("{who}: {text}\n"));
            }
        }
    }
    Ok((talk, start + complete as u64))
}

struct Session {
    project_id: i64,
    offset: u64,
    transcript: String,
}

fn get_session(db: &Db, session_id: &str) -> Result<Option<Session>> {
    Ok(db
        .conn
        .query_row("SELECT project_id, offset, transcript FROM sessions WHERE session_id = ?1", [session_id], |r| {
            Ok(Session { project_id: r.get(0)?, offset: r.get::<_, i64>(1)? as u64, transcript: r.get(2)? })
        })
        .optional()?)
}

/// Records the hook and says whether it's time to summarize. Never calls the model itself.
pub fn on_hook(db: &mut Db, input: &HookInput, _now: i64) -> Result<HookAction> {
    let Some(project) = project_for(db, &input.cwd) else { return Ok(HookAction::Nothing) };
    let offset = get_session(db, &input.session_id)?.map_or(0, |s| s.offset);
    let pending = match new_talk(Path::new(&input.transcript_path), offset) {
        Ok((talk, _)) => talk.chars().count(),
        Err(_) => 0,
    };
    db.conn.execute(
        "INSERT INTO sessions (session_id, project_id, offset, pending_chars, transcript) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(session_id) DO UPDATE SET project_id = ?2, pending_chars = ?4, transcript = ?5",
        params![input.session_id, project.id, offset as i64, pending as i64, input.transcript_path],
    )?;
    let due = match input.hook_event_name.as_str() {
        "Stop" => pending >= STOP_THRESHOLD,
        "PreCompact" | "SessionEnd" => pending > 0,
        _ => false,
    };
    Ok(if due { HookAction::Summarize { session_id: input.session_id.clone() } } else { HookAction::Nothing })
}

pub trait Summarizer {
    /// At most a few new one-line decisions in `talk` that `done` doesn't have yet.
    fn lines(&self, done: &[String], talk: &str) -> Result<Vec<String>>;
}

/// Claude Code's `claude -p`, bare: Haiku, no tools, no MCP, no settings, no slash commands, no saved session,
/// run from an empty scratch folder on the owner's own plan.
pub struct ClaudeSummarizer {
    pub claude: PathBuf,
    pub scratch: PathBuf,
}

const PROMPT: &str = "You keep a short decision log for a software project. Below are the log's latest lines, then a new part of a coding session.\n\
List at most 3 key decisions from the new part that the log doesn't already have. Each is one short English sentence: what was decided, no code, no detail.\n\
Reply with JSON only, in this shape: {\"lines\": [\"...\"]}. If nothing new is worth keeping, reply {\"lines\": []}.";

pub fn prompt(done: &[String], talk: &str) -> String {
    let log = if done.is_empty() { "(empty)".to_string() } else { done.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n") };
    format!("{PROMPT}\n\nLog:\n{log}\n\nSession:\n{talk}")
}

impl Summarizer for ClaudeSummarizer {
    fn lines(&self, done: &[String], talk: &str) -> Result<Vec<String>> {
        std::fs::create_dir_all(&self.scratch)?;
        let mut child = Command::new(&self.claude)
            .args(["-p", "--model", "haiku", "--tools", "", "--strict-mcp-config", "--setting-sources", ""])
            .args(["--disable-slash-commands", "--no-session-persistence"])
            .current_dir(&self.scratch)
            .env_remove("ANTHROPIC_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Invalid(format!("Couldn't start claude: {e}")))?;
        child.stdin.take().expect("piped").write_all(prompt(done, talk).as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            return Err(Error::Invalid("claude -p failed".into()));
        }
        Ok(parse_lines(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// The `lines` of the first JSON object in the output that has them; chatter around it is ignored.
pub fn parse_lines(out: &str) -> Vec<String> {
    for (i, _) in out.match_indices('{') {
        let mut stream = serde_json::Deserializer::from_str(&out[i..]).into_iter::<serde_json::Value>();
        if let Some(Ok(v)) = stream.next() {
            if let Some(lines) = v.get("lines").and_then(|l| l.as_array()) {
                return lines.iter().filter_map(|l| l.as_str().map(String::from)).collect();
            }
        }
    }
    vec![]
}

/// Lowercase words only, so "Sync runs in a worker." and "sync runs in a worker" are the same line.
fn norm(s: &str) -> String {
    s.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The project with a note in the vault, made from the template when it has none or it went missing.
pub fn note_for(db: &mut Db, vault: &Path, project_id: i64, today: &str) -> Result<PathBuf> {
    let p = db.get_project(project_id)?;
    if let Some(n) = p.note.as_deref().filter(|n| Path::new(n).exists()) {
        return Ok(PathBuf::from(n));
    }
    let (path, _) = notes::ensure_note(vault, &p, today)?;
    db.set_project_note(project_id, &path.to_string_lossy())?;
    Ok(path)
}

fn pending_for(db: &Db, project_id: i64) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare("SELECT text FROM pending_done WHERE project_id = ?1 ORDER BY id")?;
    Ok(stmt.query_map([project_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?)
}

/// Summarizes a session's new talk into at most 3 done lines. Returns how many were written or queued.
pub fn summarize(db: &mut Db, vault: &Path, summarizer: &dyn Summarizer, session_id: &str, today: &str) -> Result<usize> {
    let Some(s) = get_session(db, session_id)? else { return Ok(0) };
    let (talk, end) = new_talk(Path::new(&s.transcript), s.offset)?;
    // Claim the talk before the slow model call. The claim only moves a cursor nobody else moved, so when two
    // processes race for the same talk, one gets it and the other stops here.
    let claimed = db.conn.execute(
        "UPDATE sessions SET offset = ?2, pending_chars = 0 WHERE session_id = ?1 AND offset = ?3",
        params![session_id, end as i64, s.offset as i64],
    )?;
    if claimed == 0 || talk.trim().is_empty() {
        return Ok(0);
    }
    let talk = redact(&talk);
    let chars: Vec<char> = talk.chars().collect();
    let talk: String = chars[chars.len().saturating_sub(MAX_TALK)..].iter().collect();

    let note = db.get_project(s.project_id)?.note.map(PathBuf::from).filter(|n| n.exists());
    let mut known: Vec<String> = match &note {
        Some(n) => notes::read_done(n)?.into_iter().map(|l| l.text).collect(),
        None => vec![],
    };
    known.extend(pending_for(db, s.project_id)?);
    let recent = known[known.len().saturating_sub(LOG_LINES)..].to_vec();

    let mut seen: Vec<String> = known.iter().map(|l| norm(l)).collect();
    let mut fresh = Vec::new();
    for line in summarizer.lines(&recent, &talk)? {
        let Some(line) = one_line(&line) else { continue };
        let key = norm(&line);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        fresh.push(line);
        if fresh.len() == MAX_LINES {
            break;
        }
    }

    if vault.is_dir() {
        // SQLite's write lock doubles as the lock on note writes, so two processes never edit a note at once.
        exclusive(db, |db| {
            let path = note_for(db, vault, s.project_id, today)?;
            for line in &fresh {
                notes::append_done(&path, today, line)?;
            }
            Ok(())
        })?;
    } else {
        for line in &fresh {
            db.conn.execute("INSERT INTO pending_done (project_id, date, text) VALUES (?1, ?2, ?3)", params![s.project_id, today, line])?;
        }
    }
    Ok(fresh.len())
}

/// Runs `f` inside an immediate transaction: other processes wait for it, and an error rolls it back.
fn exclusive<T>(db: &mut Db, f: impl FnOnce(&mut Db) -> Result<T>) -> Result<T> {
    db.conn.execute_batch("BEGIN IMMEDIATE")?;
    match f(db) {
        Ok(v) => {
            db.conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            let _ = db.conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Writes the done lines that waited for the vault, one at a time. Each line is removed in the same transaction
/// that writes it, so it's written once even when the app and a hook flush together, and kept if the write fails.
pub fn flush_pending(db: &mut Db, vault: &Path) -> Result<usize> {
    if !vault.is_dir() {
        return Ok(0);
    }
    let mut written = 0;
    loop {
        let done = exclusive(db, |db| {
            let row: Option<(i64, i64, String, String)> = db
                .conn
                .query_row("SELECT id, project_id, date, text FROM pending_done ORDER BY id LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .optional()?;
            let Some((id, project_id, date, text)) = row else { return Ok(true) };
            db.conn.execute("DELETE FROM pending_done WHERE id = ?1", [id])?;
            match note_for(db, vault, project_id, &date) {
                Ok(path) => {
                    notes::append_done(&path, &date, &text)?;
                    written += 1;
                }
                // The project was deleted since; its line has nowhere to go.
                Err(Error::NotFound(..)) => {}
                Err(e) => return Err(e),
            }
            Ok(false)
        })?;
        if done {
            return Ok(written);
        }
    }
}
