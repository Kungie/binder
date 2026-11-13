//! Projects: names, task prefixes, state and the note path.

use crate::model::*;
use crate::notes::note_stem;
use crate::{Db, Error, Result};
use rusqlite::{OptionalExtension, Row, params};

const COLUMNS: &str = "id, name, prefix, repo, note, state, created, agent_dir";

fn row_to_project(r: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        prefix: r.get(2)?,
        repo: r.get(3)?,
        note: r.get(4)?,
        state: ProjectState::parse(&r.get::<_, String>(5)?),
        created: r.get(6)?,
        agent_dir: r.get(7)?,
    })
}

/// Turkish letters fold to the ASCII letter they look like, so `Şirket` starts `SIR`.
fn fold(c: char) -> char {
    match c {
        'ş' | 'Ş' => 'S',
        'ı' | 'İ' => 'I',
        'ğ' | 'Ğ' => 'G',
        'ü' | 'Ü' => 'U',
        'ö' | 'Ö' => 'O',
        'ç' | 'Ç' => 'C',
        c => c,
    }
}

/// The first three letters of the name, upper-cased, numbered on until it's free. `PRJ` when the name has too few letters.
pub fn derive_prefix(name: &str, taken: &[String]) -> String {
    let letters: String = name.chars().map(fold).filter(|c| c.is_ascii_alphabetic()).take(3).collect::<String>().to_ascii_uppercase();
    let base = if letters.len() < 2 { "PRJ".to_string() } else { letters };
    let mut candidate = base.clone();
    let mut n = 2;
    while taken.iter().any(|t| t == &candidate) {
        candidate = format!("{base}{n}");
        n += 1;
    }
    candidate
}

/// Tasks without a project are `TSK-n`, so no project may use that prefix.
const RESERVED: &str = "TSK";

fn checked_prefix(given: &str) -> Result<String> {
    let p = given.trim().to_ascii_uppercase();
    if p == RESERVED {
        return Err(Error::Invalid("TSK is kept for tasks without a project.".into()));
    }
    if (2..=5).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(p)
    } else {
        Err(Error::Invalid("A prefix is 2 to 5 letters or digits.".into()))
    }
}

fn checked_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() { Err(Error::Invalid("A project needs a name.".into())) } else { Ok(n.to_string()) }
}

impl Db {
    fn prefixes(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT prefix FROM projects")?;
        Ok(stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    /// Whether another project has this name, or one that would share its note file, ignoring case in any script.
    fn name_taken(&self, name: &str, except: Option<i64>) -> Result<bool> {
        let (lower, stem) = (name.to_lowercase(), note_stem(name).to_lowercase());
        let mut stmt = self.conn.prepare("SELECT id, name FROM projects")?;
        let others = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(others.into_iter().any(|(id, other)| Some(id) != except && (other.to_lowercase() == lower || note_stem(&other).to_lowercase() == stem)))
    }

    /// One past the highest number tasks already use with this prefix, so a recreated project never reuses a key.
    fn next_free_number(&self, prefix: &str) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT key FROM tasks WHERE key LIKE ?1")?;
        let keys = stmt.query_map([format!("{prefix}-%")], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let max = keys.iter().filter_map(|k| k.strip_prefix(prefix)?.strip_prefix('-')?.parse::<i64>().ok()).max().unwrap_or(0);
        Ok(max + 1)
    }

    pub fn create_project(&mut self, p: NewProject, now: i64) -> Result<Project> {
        let name = checked_name(&p.name)?;
        if self.name_taken(&name, None)? {
            return Err(Error::Invalid(format!("There is already a project called {name}, or one whose note would share its file.")));
        }
        let mut taken = self.prefixes()?;
        taken.push(RESERVED.to_string());
        let prefix = match p.prefix.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(given) => {
                let prefix = checked_prefix(given)?;
                if taken.contains(&prefix) {
                    return Err(Error::Invalid(format!("The prefix {prefix} is taken.")));
                }
                prefix
            }
            None => derive_prefix(&name, &taken),
        };
        let next = self.next_free_number(&prefix)?;
        self.conn.execute(
            "INSERT INTO projects (name, prefix, repo, state, next_number, created) VALUES (?1, ?2, ?3, 'active', ?4, ?5)",
            params![name, prefix, p.repo, next, now],
        )?;
        self.get_project(self.conn.last_insert_rowid())
    }

    pub fn get_project(&self, id: i64) -> Result<Project> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"), [id], row_to_project)
            .optional()?
            .ok_or(Error::NotFound("project", id))
    }

    /// Active first, then paused, then done; each by name.
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM projects ORDER BY CASE state WHEN 'active' THEN 0 WHEN 'paused' THEN 1 ELSE 2 END, name COLLATE NOCASE"
        ))?;
        Ok(stmt.query_map([], row_to_project)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn update_project(&mut self, id: i64, patch: ProjectPatch) -> Result<Project> {
        let mut p = self.get_project(id)?;
        if let Some(name) = patch.name {
            let name = checked_name(&name)?;
            if self.name_taken(&name, Some(id))? {
                return Err(Error::Invalid(format!("There is already a project called {name}, or one whose note would share its file.")));
            }
            p.name = name;
        }
        if patch.clear_repo {
            p.repo = None;
        } else if let Some(repo) = patch.repo {
            p.repo = Some(repo);
        }
        if let Some(state) = patch.state {
            p.state = state;
        }
        if let Some(dir) = patch.agent_dir {
            p.agent_dir = Some(dir.trim().to_string()).filter(|d| !d.is_empty());
        }
        self.conn.execute(
            "UPDATE projects SET name = ?2, repo = ?3, state = ?4, agent_dir = ?5 WHERE id = ?1",
            params![id, p.name, p.repo, p.state.as_str(), p.agent_dir],
        )?;
        self.get_project(id)
    }

    pub fn set_project_note(&mut self, id: i64, path: &str) -> Result<()> {
        if self.conn.execute("UPDATE projects SET note = ?2 WHERE id = ?1", params![id, path])? == 0 {
            return Err(Error::NotFound("project", id));
        }
        Ok(())
    }

    /// Its tasks stay, keep their keys and lose the project.
    pub fn delete_project(&mut self, id: i64) -> Result<()> {
        if self.conn.execute("DELETE FROM projects WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("project", id));
        }
        Ok(())
    }
}
