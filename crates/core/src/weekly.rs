//! The weekly note: once an ISO week has ended, a summary of it goes to `Weekly/<year>-W<week>.md` in the vault.
//! A note that exists is never written over; without the vault, the week waits.

use crate::agent::STOPPED;
use crate::events::EventKind;
use crate::model::Project;
use crate::{Db, Result};
use chrono::{Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Weekday};
use rusqlite::params;
use std::fmt::Write as _;
use std::path::Path;

const LAST: &str = "weekly_last";
const BACK: usize = 4;
const NOTHING: &str = "Nothing this week.";
const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn monday(year: i32, week: u32) -> NaiveDate {
    NaiveDate::from_isoywd_opt(year, week, Weekday::Mon).expect("an ISO week")
}

/// Weeks that have ended and have no note yet, oldest first: after `last`, at most the four latest. With no
/// `last`, only the week before this one.
pub fn due(last: Option<(i32, u32)>, today: NaiveDate) -> Vec<(i32, u32)> {
    let this = today.iso_week();
    let mut day = monday(this.year(), this.week()) - Duration::days(7);
    let mut out = vec![];
    let limit = if last.is_some() { BACK } else { 1 };
    while out.len() < limit {
        let w = day.iso_week();
        let key = (w.year(), w.week());
        if last.is_some_and(|l| key <= l) {
            break;
        }
        out.push(key);
        day -= Duration::days(7);
    }
    out.reverse();
    out
}

fn bounds(year: i32, week: u32, offset: FixedOffset) -> (i64, i64, NaiveDate, NaiveDate) {
    let start = monday(year, week);
    let end = start + Duration::days(7);
    let ts = |d: NaiveDate| offset.from_local_datetime(&d.and_hms_opt(0, 0, 0).expect("midnight")).single().expect("a fixed offset").timestamp();
    (ts(start), ts(end), start, end - Duration::days(1))
}

fn span(a: NaiveDate, b: NaiveDate) -> String {
    let m = |d: NaiveDate| MONTHS[d.month0() as usize];
    if a.month() == b.month() { format!("{} to {} {}", a.day(), b.day(), m(b)) } else { format!("{} {} to {} {}", a.day(), m(a), b.day(), m(b)) }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

pub fn render(db: &Db, projects: &[Project], year: i32, week: u32, offset: FixedOffset) -> Result<String> {
    let (from, to, first, last) = bounds(year, week, offset);
    let mut s = format!("# Week {week}, {year}\n\n{}\n\n## Done\n", span(first, last));

    let mut st = db.conn.prepare("SELECT project_id, time, text FROM done_log WHERE date >= ?1 AND date <= ?2 ORDER BY date, COALESCE(time, ''), id")?;
    let lines: Vec<(i64, Option<String>, String)> =
        st.query_map(params![first.to_string(), last.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut any = false;
    for p in projects {
        let mine: Vec<_> = lines.iter().filter(|l| l.0 == p.id).collect();
        if mine.is_empty() {
            continue;
        }
        any = true;
        let _ = writeln!(s, "### {}", p.name);
        for (_, time, text) in mine {
            let _ = match time {
                Some(t) => writeln!(s, "- {t} · {text}"),
                None => writeln!(s, "- {text}"),
            };
        }
    }
    if !any {
        let _ = writeln!(s, "{NOTHING}");
    }

    s.push_str("\n## Closed\n");
    let mut st = db.conn.prepare(
        "SELECT key, title, status FROM tasks WHERE status IN ('done', 'canceled') AND COALESCE(closed, updated) >= ?1 AND COALESCE(closed, updated) < ?2
         ORDER BY COALESCE(closed, updated), id",
    )?;
    let closed: Vec<(String, String, String)> = st.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
    if closed.is_empty() {
        let _ = writeln!(s, "{NOTHING}");
    }
    for (key, title, status) in &closed {
        let _ = writeln!(s, "- {key} {title} ({})", if status == "done" { "Done" } else { "Canceled" });
    }

    s.push_str("\n## Agent runs\n");
    let mut st = db.conn.prepare(
        "SELECT t.key, r.exit_code FROM agent_runs r JOIN tasks t ON t.id = r.task_id
         WHERE r.ended >= ?1 AND r.ended < ?2 AND COALESCE(r.exit_code, -1) != ?3 ORDER BY r.ended, r.id",
    )?;
    let runs: Vec<(String, Option<i32>)> = st.query_map(params![from, to, STOPPED], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    if runs.is_empty() {
        let _ = writeln!(s, "{NOTHING}");
    } else {
        let failed = runs.iter().filter(|r| r.1 != Some(0)).count();
        let list: Vec<String> = runs.iter().map(|(k, c)| if *c == Some(0) { k.clone() } else { format!("{k} (failed)") }).collect();
        let _ = writeln!(s, "{}, {failed} failed: {}", plural(runs.len(), "run"), list.join(", "));
    }

    s.push_str("\n## Server and printer\n");
    let events = db.events_between(from, to)?;
    let alerts = events.iter().filter(|e| matches!(e.kind, EventKind::ContainerStopped | EventKind::ContainerRestarting | EventKind::ContainerUnhealthy)).count();
    let prints = events.iter().filter(|e| matches!(e.kind, EventKind::PrintDone | EventKind::PrintFailed)).count();
    if alerts + prints == 0 {
        let _ = writeln!(s, "{NOTHING}");
    } else {
        let _ = writeln!(s, "{}, {}", plural(alerts, "container alert"), plural(prints, "print"));
    }
    Ok(s)
}

fn last_written(db: &Db) -> Result<Option<(i32, u32)>> {
    Ok(db.meta(LAST)?.and_then(|v| {
        let (y, w) = v.split_once("-W")?;
        Some((y.parse().ok()?, w.parse().ok()?))
    }))
}

/// Writes the notes that are due; returns how many files it made. An existing file counts as written.
pub fn write_due(db: &mut Db, vault: &Path, today: NaiveDate, offset: FixedOffset) -> Result<usize> {
    if !vault.is_dir() {
        return Ok(0);
    }
    let weeks = due(last_written(db)?, today);
    if weeks.is_empty() {
        return Ok(0);
    }
    let dir = vault.join("Weekly");
    std::fs::create_dir_all(&dir)?;
    let projects = db.list_projects()?;
    let mut made = 0;
    for (year, week) in weeks {
        let path = dir.join(format!("{year}-W{week:02}.md"));
        if !path.exists() {
            use std::io::Write;
            let text = render(db, &projects, year, week, offset)?;
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut f) => {
                    // A write cut short (the share dropped) takes the file back, or the next round would call a
                    // half note written.
                    if let Err(e) = f.write_all(text.as_bytes()).and_then(|_| f.sync_all()) {
                        drop(f);
                        let _ = std::fs::remove_file(&path);
                        return Err(e.into());
                    }
                    made += 1;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        db.set_meta(LAST, &format!("{year}-W{week:02}"))?;
    }
    Ok(made)
}
