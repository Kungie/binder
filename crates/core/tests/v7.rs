use binder_core::Db;
use binder_core::model::TaskView;

#[test]
fn v7_database_migrates_to_v8() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    {
        drop(Db::open(&path).unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "DROP TABLE events; DROP TABLE hidden_issues; DROP INDEX tasks_issue;
             ALTER TABLE tasks DROP COLUMN issue_listed; ALTER TABLE tasks DROP COLUMN closed; ALTER TABLE tasks DROP COLUMN issue_repo;
             ALTER TABLE tasks DROP COLUMN issue_number; ALTER TABLE tasks DROP COLUMN issue_url;
             ALTER TABLE projects DROP COLUMN open_prs; ALTER TABLE projects DROP COLUMN ci; ALTER TABLE agent_runs DROP COLUMN night;
             INSERT INTO tasks (key, title, notes, status, priority, assignee, created, updated) VALUES ('TSK-1', 'old', '', 'done', 0, 'me', 1, 5);
             PRAGMA user_version = 7;",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(version, 9);
    let tasks = db.list_tasks(TaskView::All, true).unwrap();
    assert_eq!((tasks[0].title.as_str(), tasks[0].closed, tasks[0].issue_number), ("old", None, None));
}
