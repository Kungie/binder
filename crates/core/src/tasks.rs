//! Tasks: keys, defaults, ordering and views.

use crate::model::*;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};

const COLUMNS: &str = "id, key, title, notes, status, priority, project_id, assignee, due, created, updated, due_time, attention, sort, night, closed, issue_number, issue_url, repeats";

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
        night: r.get(14)?,
        closed: r.get(15)?,
        issue_number: r.get(16)?,
        issue_url: r.get(17)?,
        repeat: r.get::<_, Option<String>>(18)?.as_deref().and_then(Repeat::parse),
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
        let night = t.night.unwrap_or(false);
        let assignee = if night { Assignee::Agent } else { t.assignee.unwrap_or_default() };
        let closed = matches!(status, Status::Done | Status::Canceled).then_some(now);
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
            "INSERT INTO tasks (key, title, notes, status, priority, project_id, assignee, due, created, updated, due_time, sort, night, closed, repeats)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                key,
                title,
                t.notes.unwrap_or_default(),
                status.as_str(),
                t.priority.unwrap_or_default().as_i64(),
                t.project_id,
                assignee.as_str(),
                t.due,
                now,
                due_time,
                sort,
                night,
                closed,
                t.repeat.map(Repeat::as_str)
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
            TaskView::Night => filters.push("night = 1"),
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
        let finished = p.status == Some(Status::Done) && t.status != Status::Done;
        if let Some(title) = p.title {
            t.title = title_of(&title)?;
        }
        if let Some(notes) = p.notes {
            t.notes = notes;
        }
        if let Some(s) = p.status {
            let was_closed = matches!(t.status, Status::Done | Status::Canceled);
            let is_closed = matches!(s, Status::Done | Status::Canceled);
            if is_closed && !was_closed {
                t.closed = Some(now);
            } else if !is_closed {
                t.closed = None;
            }
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
            // Night tasks are the agent's: giving one to the owner ends its night.
            if a == Assignee::Me {
                t.night = false;
            }
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
        if let Some(n) = p.night {
            t.night = n;
            if n {
                t.assignee = Assignee::Agent;
            }
        }
        if p.clear_repeat {
            t.repeat = None;
        } else if let Some(r) = p.repeat {
            t.repeat = Some(r);
        }
        // A finished repeating task hands its rule to the next one, so finishing it again brings no second copy.
        let next = if finished { t.repeat.take() } else { None };
        self.conn.execute(
            "UPDATE tasks SET title = ?2, notes = ?3, status = ?4, priority = ?5, project_id = ?6, assignee = ?7, due = ?8, updated = ?9,
             due_time = ?10, attention = ?11, sort = ?12, night = ?13, closed = ?14, repeats = ?15 WHERE id = ?1",
            params![
                id,
                t.title,
                t.notes,
                t.status.as_str(),
                t.priority.as_i64(),
                t.project_id,
                t.assignee.as_str(),
                t.due,
                now,
                t.due_time,
                t.attention,
                t.sort,
                t.night,
                t.closed,
                t.repeat.map(Repeat::as_str)
            ],
        )?;
        if let Some(repeat) = next {
            self.repeat_task(&t, repeat, now)?;
        }
        self.get_task(id)
    }

    /// The next one of a repeating task, due on the rule's next day.
    fn repeat_task(&mut self, t: &Task, repeat: Repeat, now: i64) -> Result<Task> {
        use chrono::TimeZone;
        let today = chrono::Local.timestamp_opt(now, 0).single().map(|d| d.date_naive()).unwrap_or_default();
        let due = t.due.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let next = next_due(repeat, due, today).format("%Y-%m-%d").to_string();
        self.create_task(
            NewTask {
                title: t.title.clone(),
                notes: Some(t.notes.clone()),
                status: Some(Status::Todo),
                priority: Some(t.priority),
                project_id: t.project_id,
                assignee: Some(t.assignee),
                due: Some(next),
                due_time: t.due_time.clone(),
                night: Some(t.night),
                repeat: Some(repeat),
            },
            now,
        )
    }

    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        // A task that came from a GitHub issue keeps the issue out of later syncs.
        self.conn.execute(
            "INSERT OR IGNORE INTO hidden_issues (issue_repo, issue_number)
             SELECT issue_repo, issue_number FROM tasks WHERE id = ?1 AND issue_repo IS NOT NULL AND issue_number IS NOT NULL",
            [id],
        )?;
        if self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("task", id));
        }
        Ok(())
    }

    /// Ties a task to the GitHub issue it came from.
    pub fn link_issue(&mut self, id: i64, repo: &str, number: i64, url: &str) -> Result<()> {
        self.conn.execute("UPDATE tasks SET issue_repo = ?2, issue_number = ?3, issue_url = ?4 WHERE id = ?1", params![id, repo, number, url])?;
        Ok(())
    }

    /// Whether the owner deleted the task an issue made, so sync leaves the issue out.
    pub fn issue_hidden(&self, repo: &str, number: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM hidden_issues WHERE issue_repo = ?1 AND issue_number = ?2", params![repo, number], |r| r.get(0))?;
        Ok(n > 0)
    }

    /// The task an issue made, if there is one.
    pub fn issue_task_id(&self, repo: &str, number: i64) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM tasks WHERE lower(issue_repo) = lower(?1) AND issue_number = ?2", params![repo, number], |r| r.get(0))
            .optional()?)
    }
}
