//! The SQLite file and its migrations.

use crate::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    pub(crate) conn: Connection,
}

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE projects (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        prefix TEXT NOT NULL UNIQUE,
        repo TEXT,
        note TEXT,
        state TEXT NOT NULL DEFAULT 'active',
        next_number INTEGER NOT NULL DEFAULT 1,
        created INTEGER NOT NULL
    );
    CREATE TABLE tasks (
        id INTEGER PRIMARY KEY,
        key TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        notes TEXT NOT NULL DEFAULT '',
        status TEXT NOT NULL,
        priority INTEGER NOT NULL DEFAULT 0,
        project_id INTEGER REFERENCES projects(id) ON DELETE SET NULL,
        assignee TEXT NOT NULL DEFAULT 'me',
        due TEXT,
        created INTEGER NOT NULL,
        updated INTEGER NOT NULL
    );
    CREATE TABLE counters (name TEXT PRIMARY KEY, next INTEGER NOT NULL);",
];

impl Db {
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Db::setup(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Db> {
        Db::setup(Connection::open_in_memory()?)
    }

    fn setup(conn: Connection) -> Result<Db> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            conn.execute_batch(&format!("BEGIN; {sql}; PRAGMA user_version = {}; COMMIT;", i + 1))?;
        }
        Ok(Db { conn })
    }
}
