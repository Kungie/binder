//! The agent picks up its own tasks: one round polls the runs still going and, when none is left, starts the
//! next task assigned to the agent, the same way the Run button does.

use crate::agent::{self, Access, AgentRun};
use crate::model::Task;
use crate::remote::Remote;
use crate::{Db, Result};
use rusqlite::OptionalExtension;
use std::sync::Mutex;

/// The next task to start and its project's agent folder: assigned to the agent, in Todo, not a night task,
/// in a project with an agent folder, never run before. Highest priority first (no priority last), then oldest.
pub fn next_task(db: &Db) -> Result<Option<(Task, String)>> {
    let pick: Option<(i64, String)> = db
        .conn
        .query_row(
            "SELECT t.id, p.agent_dir FROM tasks t JOIN projects p ON p.id = t.project_id
             WHERE t.assignee = 'agent' AND t.status = 'todo' AND t.night = 0
               AND p.agent_dir IS NOT NULL AND p.agent_dir != ''
               AND NOT EXISTS (SELECT 1 FROM agent_runs r WHERE r.task_id = t.id)
             ORDER BY CASE WHEN t.priority = 0 THEN 1 ELSE 0 END, t.priority DESC, t.created, t.id
             LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match pick {
        Some((id, dir)) => Ok(Some((db.get_task(id)?, dir))),
        None => Ok(None),
    }
}

#[derive(Debug, Default)]
pub struct Round {
    /// Runs read this round.
    pub polled: usize,
    pub started: Option<AgentRun>,
}

fn live_runs(db: &Mutex<Db>) -> Result<Vec<String>> {
    let db = db.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = db.conn.prepare("SELECT run FROM agent_runs WHERE ended IS NULL AND queued = 0 ORDER BY started")?;
    let runs = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(runs)
}

/// One round: reads every run still going, then starts the next task when `pick_up` is on and nothing runs.
/// Only one run at a time, the owner's own included.
pub fn round(db: &Mutex<Db>, remote: &dyn Remote, now: i64, pick_up: bool) -> Result<Round> {
    let mut out = Round::default();
    let mut still = 0;
    for run in live_runs(db)? {
        out.polled += 1;
        if agent::poll_run(db, remote, &run, now)?.ended.is_none() {
            still += 1;
        }
    }
    if !pick_up || still > 0 {
        return Ok(out);
    }
    let next = next_task(&db.lock().unwrap_or_else(|e| e.into_inner()))?;
    if let Some((task, dir)) = next {
        out.started = Some(agent::start_run(db, remote, task.id, &dir, Access::EditsOnly, now)?);
    }
    Ok(out)
}
