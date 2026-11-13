use binder_core::Db;

#[test]
fn v4_database_migrates_to_v5() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    // A real v4 file: today's schema with the v5 parts taken out again, and rows written before v5.
    {
        let db = Db::open(&path).unwrap();
        drop(db);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "DROP TABLE hidden_repos;
             DROP TABLE meta;
             ALTER TABLE pending_done DROP COLUMN time;
             INSERT INTO projects (name, prefix, created) VALUES ('orbit', 'ORB', 1);
             INSERT INTO pending_done (project_id, date, text) VALUES (1, '2026-11-12', 'old line');
             PRAGMA user_version = 4;",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(version, 5);
    assert_eq!(db.list_projects().unwrap()[0].name, "orbit");
    let (text, time): (String, Option<String>) = conn.query_row("SELECT text, time FROM pending_done", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((text.as_str(), time), ("old line", None));
    assert!(db.list_hidden_repos().unwrap().is_empty());
    assert_eq!(db.meta("anything").unwrap(), None);
}
