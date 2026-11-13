//! Runs of Claude Code on the home server for tasks assigned to the agent.
//! A run is `claude -p` started with `setsid nohup` in a folder on the server, so it outlives the SSH call.
//! Its files live in `~/.binder-agent/<run>.{prompt,log,pid,exit}`.

use crate::model::{Status, TaskPatch};
use crate::remote::{Remote, quote};
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    #[default]
    EditsOnly,
    Full,
}

impl Access {
    fn as_str(self) -> &'static str {
        match self {
            Access::EditsOnly => "edits_only",
            Access::Full => "full",
        }
    }

    fn flag(self) -> &'static str {
        match self {
            Access::EditsOnly => "--permission-mode acceptEdits",
            Access::Full => "--dangerously-skip-permissions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: i64,
    pub task_id: i64,
    pub run: String,
    pub dir: String,
    pub access: Access,
    pub started: i64,
    pub ended: Option<i64>,
    pub exit_code: Option<i32>,
    pub tail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    Running,
    Exited(i32),
    /// The process is gone and left no exit code.
    Lost,
}

impl AgentRun {
    pub fn state(&self) -> RunState {
        match (self.ended, self.exit_code) {
            (None, _) => RunState::Running,
            (Some(_), Some(code)) => RunState::Exited(code),
            (Some(_), None) => RunState::Lost,
        }
    }
}

/// Exit code recorded when Binder stops a run (SIGTERM).
pub const STOPPED: i32 = 143;

const COLUMNS: &str = "id, task_id, run, dir, access, started, ended, exit_code, tail";

fn row_to_run(r: &Row) -> rusqlite::Result<AgentRun> {
    Ok(AgentRun {
        id: r.get(0)?,
        task_id: r.get(1)?,
        run: r.get(2)?,
        dir: r.get(3)?,
        access: if r.get::<_, String>(4)? == "full" { Access::Full } else { Access::EditsOnly },
        started: r.get(5)?,
        ended: r.get(6)?,
        exit_code: r.get(7)?,
        tail: r.get(8)?,
    })
}

/// A folder for `cd` on the server: `~` and `~/…` keep pointing at the home folder, everything else is quoted whole.
fn shell_dir(dir: &str) -> String {
    let dir = dir.trim();
    match dir {
        "" | "~" => "\"$HOME\"".into(),
        _ => match dir.strip_prefix("~/") {
            Some(rest) => format!("\"$HOME\"/{}", quote(rest)),
            None => quote(dir),
        },
    }
}

fn files(run: &str) -> String {
    format!("\"$HOME\"/.binder-agent/{run}")
}

fn get_run(db: &Db, run: &str) -> Result<AgentRun> {
    db.conn
        .query_row(&format!("SELECT {COLUMNS} FROM agent_runs WHERE run = ?1"), [run], row_to_run)
        .optional()?
        .ok_or_else(|| Error::Invalid(format!("No run called {run}.")))
}

/// A task's runs, newest first.
pub fn list_runs(db: &Db, task_id: i64) -> Result<Vec<AgentRun>> {
    let mut stmt = db.conn.prepare(&format!("SELECT {COLUMNS} FROM agent_runs WHERE task_id = ?1 ORDER BY started DESC, id DESC"))?;
    Ok(stmt.query_map([task_id], row_to_run)?.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn set_status(db: &mut Db, task_id: i64, status: Status, now: i64) -> Result<()> {
    db.update_task(task_id, TaskPatch { status: Some(status), ..Default::default() }, now).map(|_| ())
}

fn lock(db: &Mutex<Db>) -> MutexGuard<'_, Db> {
    db.lock().unwrap_or_else(|e| e.into_inner())
}

// The database is locked only to read and write, never during an SSH call: a slow or stalled server must not
// hold up the rest of the app.

pub fn start_run(db: &Mutex<Db>, remote: &dyn Remote, task_id: i64, dir: &str, access: Access, now: i64) -> Result<AgentRun> {
    let task = lock(db).get_task(task_id)?;
    let run = format!("{}-{now}", task.key).to_lowercase();
    if get_run(&lock(db), &run).is_ok() {
        return Err(Error::Invalid("That task just started a run.".into()));
    }
    let prompt = if task.notes.trim().is_empty() { task.title.clone() } else { format!("{}\n\n{}", task.title, task.notes) };
    let f = files(&run);
    let script = format!(
        "mkdir -p \"$HOME\"/.binder-agent && cat > {f}.prompt\n\
         command -v claude >/dev/null || {{ echo \"Claude Code isn't installed on this server.\" >&2; exit 3; }}\n\
         cd {dir} && setsid nohup sh -c 'claude -p {flag} < {f}.prompt > {f}.log 2>&1; echo $? > {f}.exit' >/dev/null 2>&1 & echo $! > {f}.pid\n",
        dir = shell_dir(dir),
        flag = access.flag(),
    );
    remote.run(&script, Some(&prompt))?;
    let mut db = lock(db);
    db.conn.execute(
        "INSERT INTO agent_runs (task_id, run, dir, access, started) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![task_id, run, dir, access.as_str(), now],
    )?;
    set_status(&mut db, task_id, Status::InProgress, now)?;
    get_run(&db, &run)
}

/// Reads a live run once: its exit file, whether its process is alive, and the end of its log.
/// A run that already ended is returned as it is.
pub fn poll_run(db: &Mutex<Db>, remote: &dyn Remote, run: &str, now: i64) -> Result<AgentRun> {
    let r = get_run(&lock(db), run)?;
    if r.ended.is_some() {
        return Ok(r);
    }
    let f = files(run);
    // A login shell may print first, so the answer starts after a marker.
    let script = format!(
        "echo --binder:state--\n\
         if [ -f {f}.exit ]; then echo \"exit:$(cat {f}.exit)\"; \
         elif kill -0 \"$(cat {f}.pid 2>/dev/null)\" 2>/dev/null; then echo running; else echo lost; fi\n\
         echo --binder:tail--\ntail -n 40 {f}.log 2>/dev/null\nexit 0\n"
    );
    let out = remote.run(&script, None)?;
    let out = out.rsplit_once("--binder:state--").map_or(out.as_str(), |(_, after)| after);
    let (head, tail) = out.split_once("--binder:tail--").unwrap_or((out, ""));
    let tail = tail.trim_matches('\n').trim_end().to_string();
    let head = head.trim();
    let (ended, code) = if head == "running" {
        (None, None)
    } else if let Some(code) = head.strip_prefix("exit:") {
        (Some(now), Some(code.trim().parse::<i32>().unwrap_or(1)))
    } else {
        (Some(now), None)
    };
    let mut db = lock(db);
    db.conn.execute("UPDATE agent_runs SET ended = ?2, exit_code = ?3, tail = ?4 WHERE run = ?1 AND ended IS NULL", params![run, ended, code, tail])?;
    if ended.is_some() {
        set_status(&mut db, r.task_id, if code == Some(0) { Status::Done } else { Status::Todo }, now)?;
        db.update_task(r.task_id, TaskPatch { attention: Some(true), ..Default::default() }, now)?;
    }
    get_run(&db, run)
}

/// Stops a live run and everything it started.
pub fn stop_run(db: &Mutex<Db>, remote: &dyn Remote, run: &str, now: i64) -> Result<AgentRun> {
    let r = get_run(&lock(db), run)?;
    if r.ended.is_some() {
        return Ok(r);
    }
    let f = files(run);
    remote.run(&format!("kill -- -\"$(cat {f}.pid)\" 2>/dev/null\nexit 0\n"), None)?;
    let mut db = lock(db);
    db.conn.execute("UPDATE agent_runs SET ended = ?2, exit_code = ?3 WHERE run = ?1 AND ended IS NULL", params![run, now, STOPPED])?;
    set_status(&mut db, r.task_id, Status::Todo, now)?;
    get_run(&db, run)
}
