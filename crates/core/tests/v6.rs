use binder_core::Db;
use binder_core::model::TaskView;

#[test]
fn v5_database_migrates_to_v6() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binder.db");
    // A real v5 file: today's schema without the v6 columns, and a task written before v6.
    {
        drop(Db::open(&path).unwrap());
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "ALTER TABLE tasks DROP COLUMN night;
             ALTER TABLE agent_runs DROP COLUMN queued;
             DROP TABLE events; DROP TABLE hidden_issues; DROP INDEX tasks_issue;
             DROP TABLE prints; ALTER TABLE tasks DROP COLUMN reminded; DROP TABLE site_checks; DROP TABLE pomodoros; ALTER TABLE tasks DROP COLUMN repeats; ALTER TABLE tasks DROP COLUMN issue_listed; ALTER TABLE tasks DROP COLUMN closed; ALTER TABLE tasks DROP COLUMN issue_repo;
             ALTER TABLE tasks DROP COLUMN issue_number; ALTER TABLE tasks DROP COLUMN issue_url;
             ALTER TABLE projects DROP COLUMN open_prs; ALTER TABLE projects DROP COLUMN ci; ALTER TABLE agent_runs DROP COLUMN night;
             DROP TABLE done_log;
             INSERT INTO tasks (key, title, notes, status, priority, assignee, created, updated) VALUES ('TSK-1', 'old', '', 'todo', 0, 'agent', 1, 1);
             PRAGMA user_version = 5;",
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(version, 12);
    let tasks = db.list_tasks(TaskView::All, true).unwrap();
    assert_eq!((tasks[0].title.as_str(), tasks[0].night), ("old", false));
    assert!(db.list_tasks(TaskView::Night, true).unwrap().is_empty());
}
