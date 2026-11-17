//! Each site's checks, kept a week: the uptime bars on Sites, and down and back for the brief and notifications.

use crate::events::EventKind;
use crate::{Db, Result};
use rusqlite::params;
use serde::Serialize;
use std::collections::BTreeMap;

pub fn record(db: &mut Db, site: &str, at: i64, ok: bool, ms: Option<u32>) -> Result<()> {
    db.conn.execute("INSERT INTO site_checks (at, site, ok, ms) VALUES (?1, ?2, ?3, ?4)", params![at, site, ok, ms])?;
    Ok(())
}

/// Drops checks from before `before`; returns how many went.
pub fn prune(db: &mut Db, before: i64) -> Result<usize> {
    Ok(db.conn.execute("DELETE FROM site_checks WHERE at < ?1", [before])?)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    /// Oldest first; `None` with no check, `Some(false)` when any check in it failed.
    pub buckets: Vec<Option<bool>>,
    /// Share of good checks over the whole span.
    pub percent: Option<f32>,
    /// The last good check's answer time.
    pub ms: Option<u32>,
}

/// The last `span` seconds before `now` in `n` equal buckets.
pub fn summary(db: &Db, site: &str, now: i64, span: i64, n: usize) -> Result<Summary> {
    let from = now - span;
    let rows: Vec<(i64, bool, Option<u32>)> = db
        .conn
        .prepare("SELECT at, ok, ms FROM site_checks WHERE site = ?1 AND at >= ?2 AND at < ?3 ORDER BY at")?
        .query_map(params![site, from, now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut buckets = vec![None; n];
    let width = (span / n as i64).max(1);
    for (at, ok, _) in &rows {
        let i = (((at - from) / width) as usize).min(n - 1);
        buckets[i] = Some(buckets[i].unwrap_or(true) && *ok);
    }
    let good = rows.iter().filter(|r| r.1).count();
    let percent = (!rows.is_empty()).then(|| good as f32 * 100.0 / rows.len() as f32);
    let ms = rows.iter().rev().find(|r| r.1).and_then(|r| r.2);
    Ok(Summary { buckets, percent, ms })
}

/// Failures in a row per site: the second makes a site down, the first good check after that brings it back.
#[derive(Debug, Default)]
pub struct Watch {
    fails: BTreeMap<String, u32>,
}

impl Watch {
    pub fn saw(&mut self, site: &str, ok: bool) -> Option<EventKind> {
        let fails = self.fails.entry(site.to_string()).or_default();
        if ok {
            let was_down = *fails >= 2;
            *fails = 0;
            return was_down.then_some(EventKind::SiteBack);
        }
        *fails += 1;
        (*fails == 2).then_some(EventKind::SiteDown)
    }
}
