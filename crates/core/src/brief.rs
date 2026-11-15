//! The morning brief: what happened since the owner last looked, shown on Overview from 05:00 until dismissed or
//! noon. Lines are facts with their times; the interface words them.

use crate::agent::STOPPED;
use crate::events::EventKind;
use crate::{Db, Result};
use chrono::{NaiveDateTime, NaiveTime, Timelike};
use rusqlite::params;
use serde::Serialize;

const DISMISSED: &str = "brief_dismissed";
const DAY: i64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BriefItem {
    Run { task_id: i64, key: String, ok: bool, night: bool, at: i64 },
    /// The latest problem of one container, and when it ran again after it.
    Container { name: String, problem: String, at: i64, back_at: Option<i64> },
    ServerRestart { at: i64 },
    Print { job: String, ok: bool, at: i64 },
    Issue { task_id: i64, key: String, title: String, at: i64 },
}

impl BriefItem {
    fn at(&self) -> i64 {
        match self {
            BriefItem::Run { at, .. } | BriefItem::Container { at, .. } | BriefItem::ServerRestart { at } | BriefItem::Print { at, .. } | BriefItem::Issue { at, .. } => *at,
        }
    }
}

/// Where the brief starts: the last dismiss, or a day back.
pub fn since(db: &Db, now: i64) -> Result<i64> {
    Ok(db.meta(DISMISSED)?.and_then(|v| v.parse().ok()).unwrap_or(now - DAY))
}

pub fn dismiss(db: &mut Db, now: i64) -> Result<()> {
    db.set_meta(DISMISSED, &now.to_string())
}

/// When the last dismiss was, if any; for `shown`.
pub fn dismissed_at(db: &Db) -> Result<Option<i64>> {
    Ok(db.meta(DISMISSED)?.and_then(|v| v.parse().ok()))
}

/// From 05:00 to noon local time, unless dismissed since this morning's 05:00.
pub fn shown(local_now: NaiveDateTime, dismissed: Option<NaiveDateTime>) -> bool {
    let five = local_now.date().and_time(NaiveTime::from_hms_opt(5, 0, 0).expect("05:00"));
    (5..12).contains(&local_now.hour()) && dismissed.is_none_or(|d| d < five)
}

fn problem(kind: EventKind) -> Option<&'static str> {
    match kind {
        EventKind::ContainerStopped => Some("stopped"),
        EventKind::ContainerRestarting => Some("restarting"),
        EventKind::ContainerUnhealthy => Some("unhealthy"),
        _ => None,
    }
}

/// Everything in `since <= at < now`, oldest first. Runs the owner stopped and restarts they started stay out.
pub fn items(db: &Db, since: i64, now: i64) -> Result<Vec<BriefItem>> {
    let mut out = vec![];
    let mut st = db.conn.prepare(
        "SELECT r.task_id, t.key, r.exit_code, r.night, r.ended FROM agent_runs r JOIN tasks t ON t.id = r.task_id
         WHERE r.ended >= ?1 AND r.ended < ?2 AND COALESCE(r.exit_code, -1) != ?3 ORDER BY r.ended, r.id",
    )?;
    let runs = st.query_map(params![since, now, STOPPED], |r| {
        Ok(BriefItem::Run { task_id: r.get(0)?, key: r.get(1)?, ok: r.get::<_, Option<i32>>(2)? == Some(0), night: r.get(3)?, at: r.get(4)? })
    })?;
    for run in runs {
        out.push(run?);
    }
    // Per container, the latest problem and the first time it ran again after it.
    let mut containers: Vec<BriefItem> = vec![];
    for e in db.events_between(since, now)? {
        if let Some(p) = problem(e.kind) {
            containers.retain(|c| !matches!(c, BriefItem::Container { name, .. } if *name == e.subject));
            containers.push(BriefItem::Container { name: e.subject.clone(), problem: p.into(), at: e.at, back_at: None });
            continue;
        }
        match e.kind {
            EventKind::ContainerBack => {
                if let Some(BriefItem::Container { back_at, .. }) =
                    containers.iter_mut().find(|c| matches!(c, BriefItem::Container { name, back_at: None, .. } if *name == e.subject))
                {
                    *back_at = Some(e.at);
                }
            }
            EventKind::ServerRestart => out.push(BriefItem::ServerRestart { at: e.at }),
            EventKind::PrintDone | EventKind::PrintFailed => out.push(BriefItem::Print { job: e.subject.clone(), ok: e.kind == EventKind::PrintDone, at: e.at }),
            EventKind::IssueAdded => {
                let task: Option<(i64, String)> = {
                    use rusqlite::OptionalExtension;
                    db.conn.query_row("SELECT id, title FROM tasks WHERE key = ?1", [&e.subject], |r| Ok((r.get(0)?, r.get(1)?))).optional()?
                };
                // A task deleted since has nothing to open.
                if let Some((task_id, _)) = task {
                    out.push(BriefItem::Issue { task_id, key: e.subject.clone(), title: e.detail.clone(), at: e.at });
                }
            }
            _ => {}
        }
    }
    out.extend(containers);
    out.sort_by_key(BriefItem::at);
    Ok(out)
}
