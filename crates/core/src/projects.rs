//! Projects. Task 2 needs only enough of them to file tasks under one.

use crate::model::*;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};

const COLUMNS: &str = "id, name, prefix, repo, note, state, created";

fn row_to_project(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        prefix: r.get(2)?,
        repo: r.get(3)?,
        note: r.get(4)?,
        state: ProjectState::parse(&r.get::<_, String>(5)?),
        created: r.get(6)?,
    })
}

impl Db {
    pub fn create_project(&mut self, p: NewProject, now: i64) -> Result<Project> {
        let name = p.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Invalid("A project needs a name.".into()));
        }
        let prefix = name.chars().filter(|c| c.is_ascii_alphabetic()).take(3).collect::<String>().to_uppercase();
        self.conn.execute(
            "INSERT INTO projects (name, prefix, repo, state, created) VALUES (?1, ?2, ?3, 'active', ?4)",
            params![name, prefix, p.repo, now],
        )?;
        self.get_project(self.conn.last_insert_rowid())
    }

    pub fn get_project(&self, id: i64) -> Result<Project> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"), [id], row_to_project)
            .optional()?
            .ok_or(Error::NotFound("project", id))
    }
}
