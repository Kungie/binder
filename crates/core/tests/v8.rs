use binder_core::Db;

#[test]
fn v8_database_marks_its_open_issue_tasks_listed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    {
        drop(Db::open(&path).unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "ALTER TABLE tasks DROP COLUMN repeats; ALTER TABLE tasks DROP COLUMN issue_listed;
             INSERT INTO projects (name, prefix, created) VALUES ('orbit', 'ORB', 1);
             INSERT INTO tasks (key, title, status, project_id, created, updated, issue_repo, issue_number)
               VALUES ('ORB-1', 'Open', 'todo', 1, 1, 1, 'demo-dev/orbit', 12),
                      ('ORB-2', 'Closed', 'done', 1, 1, 1, 'demo-dev/orbit', 7),
                      ('ORB-3', 'Plain', 'todo', 1, 1, 1, NULL, NULL);
             PRAGMA user_version = 8;",
        )
        .unwrap();
    }
    drop(Db::open(&path).unwrap());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(version, 10);
    let listed: Vec<(String, i64)> =
        conn.prepare("SELECT key, issue_listed FROM tasks ORDER BY key").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect();
    assert_eq!(listed, vec![("ORB-1".into(), 1), ("ORB-2".into(), 0), ("ORB-3".into(), 0)]);
}
