//! Tasks: keys, defaults, ordering and views.

use crate::model::*;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};

const COLUMNS: &str = "id, key, title, notes, status, priority, project_id, assignee, due, created, updated, due_time, attention, sort";

/// Open work first, most urgent first, most recently touched first.
const ORDER: &str = "ORDER BY CASE status WHEN 'in_progress' THEN 0 WHEN 'todo' THEN 1 WHEN 'backlog' THEN 2 WHEN 'done' THEN 3 ELSE 4 END,
    CASE WHEN priority = 0 THEN 1 ELSE 0 END, priority DESC, updated DESC, id DESC";

fn row_to_task(r: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        key: r.get(1)?,
        title: r.get(2)?,
        notes: r.get(3)?,
        status: Status::parse(&r.get::<_, String>(4)?).unwrap_or_default(),
        priority: Priority::from_i64(r.get(5)?),
        project_id: r.get(6)?,
        assignee: Assignee::parse(&r.get::<_, String>(7)?),
        due: r.get(8)?,
        created: r.get(9)?,
        updated: r.get(10)?,
        due_time: r.get(11)?,
        attention: r.get(12)?,
        sort: r.get(13)?,
    })
}

/// `HH:MM`, 24-hour.
fn time_of(time: &str) -> Result<String> {
    let t = time.trim();
    let ok = t.len() == 5
        && t.as_bytes()[2] == b':'
        && t[..2].parse::<u8>().is_ok_and(|h| h < 24)
        && t[3..].parse::<u8>().is_ok_and(|m| m < 60);
    if ok { Ok(t.to_string()) } else { Err(Error::Invalid("A time is HH:MM.".into())) }
}

fn title_of(title: &str) -> Result<String> {
    let t = title.trim();
    if t.is_empty() { Err(Error::Invalid("A task needs a title.".into())) } else { Ok(t.to_string()) }
}

impl Db {
    pub fn create_task(&mut self, t: NewTask, now: i64) -> Result<Task> {
        let title = title_of(&t.title)?;
        let due_time = match (&t.due, &t.due_time) {
            (Some(_), Some(time)) => Some(time_of(time)?),
            _ => None,
        };
        let status = t.status.unwrap_or_default();
        let tx = self.conn.transaction()?;
        let sort: i64 = tx.query_row("SELECT COALESCE(MAX(sort), 0) + 1 FROM tasks WHERE status = ?1", [status.as_str()], |r| r.get(0))?;
        let key = match t.project_id {
            Some(pid) => {
                let (prefix, n): (String, i64) = tx
                    .query_row("SELECT prefix, next_number FROM projects WHERE id = ?1", [pid], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()?
                    .ok_or(Error::NotFound("project", pid))?;
                tx.execute("UPDATE projects SET next_number = next_number + 1 WHERE id = ?1", [pid])?;
                format!("{prefix}-{n}")
            }
            None => {
                let n: i64 = tx.query_row("SELECT next FROM counters WHERE name = 'TSK'", [], |r| r.get(0)).optional()?.unwrap_or(1);
                tx.execute("INSERT INTO counters (name, next) VALUES ('TSK', ?1) ON CONFLICT(name) DO UPDATE SET next = ?1", [n + 1])?;
                format!("TSK-{n}")
            }
        };
        tx.execute(
            "INSERT INTO tasks (key, title, notes, status, priority, project_id, assignee, due, created, updated, due_time, sort)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11)",
            params![
                key,
                title,
                t.notes.unwrap_or_default(),
                status.as_str(),
                t.priority.unwrap_or_default().as_i64(),
                t.project_id,
                t.assignee.unwrap_or_default().as_str(),
                t.due,
                now,
                due_time,
                sort
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        self.get_task(id)
    }

    pub fn get_task(&self, id: i64) -> Result<Task> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1"), [id], row_to_task)
            .optional()?
            .ok_or(Error::NotFound("task", id))
    }

    pub fn list_tasks(&self, view: TaskView, include_closed: bool) -> Result<Vec<Task>> {
        let mut filters = vec![];
        let mut project = None;
        match view {
            TaskView::All => {}
            TaskView::Mine => filters.push("assignee = 'me'"),
            TaskView::Agent => filters.push("assignee = 'agent'"),
            TaskView::Project(id) => {
                filters.push("project_id = ?1");
                project = Some(id);
            }
        }
        if !include_closed {
            filters.push("status NOT IN ('done', 'canceled')");
        }
        let filter = if filters.is_empty() { String::new() } else { format!("WHERE {}", filters.join(" AND ")) };
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM tasks {filter} {ORDER}"))?;
        let rows = match project {
            Some(id) => stmt.query_map([id], row_to_task)?.collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt.query_map([], row_to_task)?.collect::<rusqlite::Result<Vec<_>>>()?,
        };
        Ok(rows)
    }

    pub fn update_task(&mut self, id: i64, p: TaskPatch, now: i64) -> Result<Task> {
        let mut t = self.get_task(id)?;
        if let Some(title) = p.title {
            t.title = title_of(&title)?;
        }
        if let Some(notes) = p.notes {
            t.notes = notes;
        }
        if let Some(s) = p.status {
            t.status = s;
        }
        if let Some(pr) = p.priority {
            t.priority = pr;
        }
        if p.clear_project {
            t.project_id = None;
        } else if let Some(pid) = p.project_id {
            self.get_project(pid)?;
            t.project_id = Some(pid);
        }
        if let Some(a) = p.assignee {
            t.assignee = a;
        }
        if p.clear_due {
            t.due = None;
            t.due_time = None;
        } else if let Some(d) = p.due {
            t.due = Some(d);
        }
        if p.clear_due_time {
            t.due_time = None;
        } else if let Some(time) = p.due_time {
            t.due_time = Some(time_of(&time)?);
        }
        if let Some(a) = p.attention {
            t.attention = a;
        }
        if let Some(s) = p.sort {
            t.sort = s;
        }
        self.conn.execute(
            "UPDATE tasks SET title = ?2, notes = ?3, status = ?4, priority = ?5, project_id = ?6, assignee = ?7, due = ?8, updated = ?9,
             due_time = ?10, attention = ?11, sort = ?12 WHERE id = ?1",
            params![id, t.title, t.notes, t.status.as_str(), t.priority.as_i64(), t.project_id, t.assignee.as_str(), t.due, now, t.due_time, t.attention, t.sort],
        )?;
        self.get_task(id)
    }

    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("task", id));
        }
        Ok(())
    }
}
