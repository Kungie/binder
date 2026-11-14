//! Night tasks wait in a queue on the server, `~/.binder-agent/night/<run>.job`, and a line in the owner's
//! crontab starts `night.sh` at 03:00, which runs them one at a time the way a run starts in the day, so Binder
//! reads them like any other run. Nothing runs from 07:00 on; the rest wait for the next night.

use crate::agent::{self, Access, AgentRun};
use crate::model::Status;
use crate::remote::{Remote, quote};
use crate::{Db, Result};
use rusqlite::params;
use std::collections::HashSet;
use std::sync::Mutex;

/// Marks Binder's line in the crontab, so it can be found and removed alone.
pub const MARK: &str = "# binder-night";
const LINE: &str = "0 3 * * * \"$HOME\"/.binder-agent/night.sh # binder-night";

/// `~/.binder-agent/night.sh`. Each job is a few `name=value` lines Binder wrote with every value quoted;
/// its prompt is `<run>.prompt` next to the day runs' files.
pub const SCRIPT: &str = r#"#!/bin/sh
# Binder's night queue: the jobs in ~/.binder-agent/night, oldest first, one at a time, from 03:00 to 07:00.
PATH="$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin"
q="$HOME/.binder-agent/night"
awake() { case "$(date +%H)" in 03|04|05|06) return 0 ;; *) return 1 ;; esac; }
awake || exit 0
mkdir "$q/.running" 2>/dev/null || exit 0
trap 'rmdir "$q/.running"' EXIT
for job in $(ls -tr "$q"/*.job 2>/dev/null); do
  awake || break
  [ -f "$job" ] || continue
  run=$(basename "$job" .job)
  f="$HOME/.binder-agent/$run"
  dir=""; flag=""
  . "$job"
  rm -f "$job"
  ( cd "$dir" && claude -p $flag < "$f.prompt" > "$f.log" 2>&1; echo $? > "$f.exit" ) &
  echo $! > "$f.pid"
  wait $!
done
"#;

fn after<'a>(out: &'a str, marker: &str) -> &'a str {
    out.rsplit_once(marker).map_or("", |(_, a)| a)
}

fn crontab(remote: &dyn Remote) -> Result<String> {
    let out = remote.run("echo --binder:crontab--\ncrontab -l 2>/dev/null\nexit 0\n", None)?;
    Ok(after(&out, "--binder:crontab--\n").to_string())
}

fn lines(tab: &str) -> Vec<&str> {
    tab.lines().filter(|l| !l.trim().is_empty()).collect()
}

/// Whether the owner's crontab has Binder's line.
pub fn cron_state(remote: &dyn Remote) -> Result<bool> {
    Ok(crontab(remote)?.lines().any(|l| l.contains(MARK)))
}

/// Writes `night.sh` and adds Binder's line to the crontab, keeping every other line.
pub fn cron_install(remote: &dyn Remote) -> Result<()> {
    let out = remote.run(
        "mkdir -p \"$HOME\"/.binder-agent/night && cat > \"$HOME\"/.binder-agent/night.sh && chmod 755 \"$HOME\"/.binder-agent/night.sh\n\
         echo --binder:crontab--\ncrontab -l 2>/dev/null\nexit 0\n",
        Some(SCRIPT),
    )?;
    let tab = after(&out, "--binder:crontab--\n");
    if tab.lines().any(|l| l.contains(MARK)) {
        return Ok(());
    }
    let mut next: Vec<&str> = lines(tab);
    next.push(LINE);
    remote.run("crontab -\n", Some(&format!("{}\n", next.join("\n"))))?;
    Ok(())
}

/// Takes Binder's line out of the crontab and leaves the rest as it was.
pub fn cron_remove(remote: &dyn Remote) -> Result<()> {
    let tab = crontab(remote)?;
    if !tab.lines().any(|l| l.contains(MARK)) {
        return Ok(());
    }
    let next: Vec<&str> = lines(&tab).into_iter().filter(|l| !l.contains(MARK)).collect();
    let body = if next.is_empty() { String::new() } else { format!("{}\n", next.join("\n")) };
    remote.run("crontab -\n", Some(&body))?;
    Ok(())
}

fn lock(db: &Mutex<Db>) -> std::sync::MutexGuard<'_, Db> {
    db.lock().unwrap_or_else(|e| e.into_inner())
}

/// Night tasks in Todo with an agent folder and no run waiting or going: they need a job.
fn to_queue(db: &Db) -> Result<Vec<(i64, String)>> {
    let mut stmt = db.conn.prepare(
        "SELECT t.id, p.agent_dir FROM tasks t JOIN projects p ON p.id = t.project_id
         WHERE t.night = 1 AND t.status = 'todo' AND p.agent_dir IS NOT NULL AND p.agent_dir != ''
           AND NOT EXISTS (SELECT 1 FROM agent_runs r WHERE r.task_id = t.id AND r.ended IS NULL)
         ORDER BY t.created, t.id",
    )?;
    Ok(stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Queued runs: those still wanted, and those whose task is no longer a night task in Todo.
fn queued_runs(db: &Db) -> Result<(HashSet<String>, Vec<String>)> {
    let mut stmt = db.conn.prepare(
        "SELECT r.run, (t.night = 1 AND t.status = 'todo') FROM agent_runs r JOIN tasks t ON t.id = r.task_id
         WHERE r.queued = 1 AND r.ended IS NULL",
    )?;
    let mut keep = HashSet::new();
    let mut drop = vec![];
    for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))? {
        let (run, wanted) = row?;
        if wanted { keep.insert(run) } else { drop.push(run); true };
    }
    Ok((keep, drop))
}

/// Makes the server's queue match the night tasks: queues the new ones, removes jobs for tasks that are no
/// longer night tasks in Todo, and jobs Binder doesn't know at all (a task deleted while queued).
pub fn sync(db: &Mutex<Db>, remote: &dyn Remote, now: i64) -> Result<()> {
    let out = remote.run("echo --binder:jobs--\ncd \"$HOME\"/.binder-agent/night 2>/dev/null && ls -1 -- *.job 2>/dev/null\nexit 0\n", None)?;
    let on_server: Vec<String> = after(&out, "--binder:jobs--\n").lines().filter_map(|l| l.trim().strip_suffix(".job")).map(String::from).collect();
    let (keep, drop) = queued_runs(&lock(db))?;
    let gone: Vec<&String> = on_server.iter().filter(|r| !keep.contains(*r)).chain(drop.iter().filter(|r| !on_server.contains(r))).collect();
    if !gone.is_empty() {
        let files: Vec<String> = gone.iter().map(|r| format!("\"$HOME\"/.binder-agent/night/{}", quote(&format!("{r}.job")))).collect();
        remote.run(&format!("rm -f {}\nexit 0\n", files.join(" ")), None)?;
    }
    {
        let db = lock(db);
        for run in &drop {
            db.conn.execute("DELETE FROM agent_runs WHERE run = ?1 AND queued = 1", [run])?;
        }
    }
    // Read first: a guard in the loop header would live through `queue`, which locks again.
    let wanted = to_queue(&lock(db))?;
    for (task_id, dir) in wanted {
        queue(db, remote, task_id, &dir, now)?;
    }
    Ok(())
}

fn queue(db: &Mutex<Db>, remote: &dyn Remote, task_id: i64, dir: &str, now: i64) -> Result<AgentRun> {
    let task = lock(db).get_task(task_id)?;
    let run = format!("{}-{now}", task.key).to_lowercase();
    let prompt = if task.notes.trim().is_empty() { task.title.clone() } else { format!("{}\n\n{}", task.title, task.notes) };
    let access = Access::EditsOnly;
    let job = [format!("dir={}", agent::shell_dir(dir)), format!("flag={}", quote(access.flag()))];
    let script = format!(
        "mkdir -p \"$HOME\"/.binder-agent/night && cat > {f}.prompt && printf '%s\\n' {lines} > \"$HOME\"/.binder-agent/night/{run}.job\n",
        f = agent::files(&run),
        lines = job.iter().map(|l| quote(l)).collect::<Vec<_>>().join(" "),
    );
    remote.run(&script, Some(&prompt))?;
    let db = lock(db);
    db.conn.execute(
        "INSERT INTO agent_runs (task_id, run, dir, access, started, queued) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![task_id, run, dir, access.as_str(), now],
    )?;
    agent::get_run(&db, &run)
}

/// Removes every queued job and its run, for when the crontab line goes away.
pub fn clear(db: &Mutex<Db>, remote: &dyn Remote) -> Result<()> {
    remote.run("rm -f \"$HOME\"/.binder-agent/night/*.job\nexit 0\n", None)?;
    lock(db).conn.execute("DELETE FROM agent_runs WHERE queued = 1 AND ended IS NULL", [])?;
    Ok(())
}

/// For a queued run: true once `night.sh` started it (it is then a run like any other, started now and
/// In progress); false while it waits. A job that vanished ends the run as lost.
pub(crate) fn started(db: &Mutex<Db>, remote: &dyn Remote, r: &AgentRun, now: i64) -> Result<bool> {
    let f = agent::files(&r.run);
    let out = remote.run(
        &format!(
            "echo --binder:state--\nif [ -f {f}.pid ]; then echo started; elif [ -f \"$HOME\"/.binder-agent/night/{run}.job ]; then echo queued; else echo gone; fi\nexit 0\n",
            run = quote(&format!("{}", r.run)),
        ),
        None,
    )?;
    let mut db = lock(db);
    match after(&out, "--binder:state--").trim() {
        "started" => {
            db.conn.execute("UPDATE agent_runs SET queued = 0, started = ?2 WHERE run = ?1", params![r.run, now])?;
            agent::set_status(&mut db, r.task_id, Status::InProgress, now)?;
            Ok(true)
        }
        "gone" => {
            db.conn.execute("UPDATE agent_runs SET queued = 0, ended = ?2 WHERE run = ?1", params![r.run, now])?;
            agent::set_status(&mut db, r.task_id, Status::Todo, now)?;
            db.update_task(r.task_id, crate::model::TaskPatch { night: Some(false), attention: Some(true), ..Default::default() }, now)?;
            Ok(false)
        }
        _ => Ok(false),
    }
}
