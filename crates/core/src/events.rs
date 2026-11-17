//! The event log: what happened while Binder ran or while the Mac slept, for the morning brief and the weekly note.
//! Events are logged whether or not their notification is on.

use crate::{Db, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ContainerStopped,
    ContainerRestarting,
    ContainerUnhealthy,
    ContainerBack,
    /// A restart the owner asked for from Binder.
    OwnerRestart,
    ServerRestart,
    PrintDone,
    PrintFailed,
    IssueAdded,
    /// A site failed two checks in a row.
    SiteDown,
    SiteBack,
    /// A disk passed 90 %; the detail is the percent.
    DiskFull,
}

const ALL: [EventKind; 12] = [
    EventKind::ContainerStopped,
    EventKind::ContainerRestarting,
    EventKind::ContainerUnhealthy,
    EventKind::ContainerBack,
    EventKind::OwnerRestart,
    EventKind::ServerRestart,
    EventKind::PrintDone,
    EventKind::PrintFailed,
    EventKind::IssueAdded,
    EventKind::SiteDown,
    EventKind::SiteBack,
    EventKind::DiskFull,
];

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::ContainerStopped => "container_stopped",
            EventKind::ContainerRestarting => "container_restarting",
            EventKind::ContainerUnhealthy => "container_unhealthy",
            EventKind::ContainerBack => "container_back",
            EventKind::OwnerRestart => "owner_restart",
            EventKind::ServerRestart => "server_restart",
            EventKind::PrintDone => "print_done",
            EventKind::PrintFailed => "print_failed",
            EventKind::IssueAdded => "issue_added",
            EventKind::SiteDown => "site_down",
            EventKind::SiteBack => "site_back",
            EventKind::DiskFull => "disk_full",
        }
    }

    pub fn parse(s: &str) -> Option<EventKind> {
        ALL.into_iter().find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Event {
    pub id: i64,
    /// Unix seconds.
    pub at: i64,
    pub kind: EventKind,
    /// What it is about: a container, a job, a task key.
    pub subject: String,
    pub detail: String,
}

impl Db {
    pub fn log_event(&mut self, at: i64, kind: EventKind, subject: &str, detail: &str) -> Result<()> {
        self.conn.execute("INSERT INTO events (at, kind, subject, detail) VALUES (?1, ?2, ?3, ?4)", params![at, kind.as_str(), subject, detail])?;
        Ok(())
    }

    /// Events with `from <= at < to`, oldest first.
    pub fn events_between(&self, from: i64, to: i64) -> Result<Vec<Event>> {
        let mut st = self.conn.prepare("SELECT id, at, kind, subject, detail FROM events WHERE at >= ?1 AND at < ?2 ORDER BY at, id")?;
        let rows = st.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?, r.get(3)?, r.get(4)?)))?;
        let mut out = vec![];
        for row in rows {
            let (id, at, kind, subject, detail) = row?;
            // A kind this build doesn't know (a newer Binder wrote it) is left out.
            if let Some(kind) = EventKind::parse(&kind) {
                out.push(Event { id, at, kind, subject, detail });
            }
        }
        Ok(out)
    }

    /// Takes back one event that turned out not to happen.
    pub fn unlog_event(&mut self, at: i64, kind: EventKind, subject: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM events WHERE id = (SELECT max(id) FROM events WHERE at = ?1 AND kind = ?2 AND subject = ?3)",
            params![at, kind.as_str(), subject],
        )?;
        Ok(())
    }

    /// Drops events from before `before`; returns how many went.
    pub fn prune_events(&mut self, before: i64) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM events WHERE at < ?1", [before])?)
    }
}
