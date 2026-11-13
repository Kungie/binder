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
    // v2: devices, agent runs, session cursors and done lines waiting for the vault
    "CREATE TABLE devices (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,
        address TEXT NOT NULL,
        port INTEGER NOT NULL DEFAULT 22,
        note TEXT NOT NULL DEFAULT '',
        created INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE agent_runs (
        id INTEGER PRIMARY KEY,
        task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        run TEXT NOT NULL UNIQUE,
        dir TEXT NOT NULL,
        access TEXT NOT NULL,
        started INTEGER NOT NULL,
        ended INTEGER,
        exit_code INTEGER,
        tail TEXT NOT NULL DEFAULT ''
    );
    ALTER TABLE projects ADD COLUMN agent_dir TEXT;
    CREATE TABLE sessions (
        session_id TEXT PRIMARY KEY,
        project_id INTEGER,
        offset INTEGER NOT NULL DEFAULT 0,
        pending_chars INTEGER NOT NULL DEFAULT 0,
        transcript TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE pending_done (
        id INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL,
        date TEXT NOT NULL,
        text TEXT NOT NULL
    );",
    // v3: board order, due times, attention, GitHub projects and colours
    "ALTER TABLE tasks ADD COLUMN due_time TEXT;
    ALTER TABLE tasks ADD COLUMN attention INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE tasks ADD COLUMN sort INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN github TEXT;
    ALTER TABLE projects ADD COLUMN url TEXT;
    ALTER TABLE projects ADD COLUMN description TEXT;
    ALTER TABLE projects ADD COLUMN pushed_at TEXT;
    ALTER TABLE projects ADD COLUMN private INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE projects ADD COLUMN color TEXT NOT NULL DEFAULT '';
    ALTER TABLE projects ADD COLUMN language TEXT;
    CREATE UNIQUE INDEX projects_github ON projects(github);",
    // v4: GitHub's repo id, which survives renames and transfers
    "ALTER TABLE projects ADD COLUMN github_id TEXT;
    CREATE UNIQUE INDEX projects_github_id ON projects(github_id);",
    // v5: repos the owner deleted, done line times, small settings
    "CREATE TABLE hidden_repos (id INTEGER PRIMARY KEY, github_id TEXT UNIQUE, name_with_owner TEXT NOT NULL, hidden_at INTEGER NOT NULL);
    ALTER TABLE pending_done ADD COLUMN time TEXT;
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
];

impl Db {
    /// The app and `binder-cli` share the file, so it uses WAL and waits up to 5 s for the other's lock.
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Db::setup(conn)
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

    /// A small stored setting, such as `last_done`.
    pub fn meta(&self, key: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> Result<()> {
        self.conn.execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2", [key, value])?;
        Ok(())
    }
}
