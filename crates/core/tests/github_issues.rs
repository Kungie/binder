use binder_core::events::EventKind;
use binder_core::github::*;
use binder_core::model::*;
use binder_core::{Db, Error, Result};
use std::cell::RefCell;

const NOW: i64 = 1_794_600_000;

struct Fake {
    repos: Vec<Repo>,
    issues: RefCell<Result<Vec<Issue>>>,
    health: RefCell<Result<Vec<Health>>>,
}

fn take<T: Clone>(r: &RefCell<Result<Vec<T>>>) -> Result<Vec<T>> {
    match &*r.borrow() {
        Ok(v) => Ok(v.clone()),
        Err(_) => Err(Error::Invalid("gh failed".into())),
    }
}

impl GitHub for Fake {
    fn login(&self) -> Result<String> {
        Ok("demo-dev".into())
    }
    fn repos(&self, _: &str) -> Result<Vec<Repo>> {
        Ok(self.repos.clone())
    }
    fn issues(&self) -> Result<Vec<Issue>> {
        take(&self.issues)
    }
    fn health(&self) -> Result<Vec<Health>> {
        take(&self.health)
    }
}

fn repo(name: &str) -> Repo {
    Repo {
        id: Some(format!("R_{name}")),
        name: name.into(),
        name_with_owner: format!("demo-dev/{name}"),
        description: None,
        url: format!("https://github.com/demo-dev/{name}"),
        is_private: false,
        is_archived: false,
        pushed_at: "2026-11-12T00:00:00Z".into(),
        language: None,
    }
}

fn issue(repo: &str, number: i64, title: &str) -> Issue {
    Issue { repo: format!("demo-dev/{repo}"), number, title: title.into(), body: format!("Body of {title}"), url: format!("https://github.com/demo-dev/{repo}/issues/{number}") }
}

fn fake(issues: Vec<Issue>) -> Fake {
    Fake { repos: vec![repo("orbit"), repo("lab")], issues: RefCell::new(Ok(issues)), health: RefCell::new(Ok(vec![])) }
}

fn issue_tasks(db: &Db) -> Vec<Task> {
    db.list_tasks(TaskView::All, true).unwrap().into_iter().filter(|t| t.issue_number.is_some()).collect()
}

#[test]
fn an_assigned_issue_becomes_a_task() {
    let mut db = Db::in_memory().unwrap();
    let report = sync(&mut db, &fake(vec![issue("orbit", 12, "Crash on start")]), NOW).unwrap();
    assert_eq!(report.issues_added, 1);
    let t = &issue_tasks(&db)[0];
    assert_eq!((t.title.as_str(), t.notes.as_str(), t.status, t.assignee), ("Crash on start", "Body of Crash on start", Status::Todo, Assignee::Me));
    assert_eq!((t.issue_number, t.issue_url.as_deref()), (Some(12), Some("https://github.com/demo-dev/orbit/issues/12")));
    let orbit = db.list_projects().unwrap().into_iter().find(|p| p.name == "orbit").unwrap();
    assert_eq!(t.project_id, Some(orbit.id));
    let events = db.events_between(0, i64::MAX).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].kind, events[0].subject.as_str(), events[0].detail.as_str()), (EventKind::IssueAdded, t.key.as_str(), "Crash on start"));
    // A second sync brings nothing new.
    assert_eq!(sync(&mut db, &fake(vec![issue("orbit", 12, "Crash on start")]), NOW).unwrap().issues_added, 0);
    assert_eq!(issue_tasks(&db).len(), 1);
}

#[test]
fn issues_in_repos_that_arent_projects_are_left_out() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![issue("elsewhere", 3, "Not mine")]), NOW).unwrap();
    assert!(issue_tasks(&db).is_empty());
}

#[test]
fn sync_never_overwrites_an_issue_task() {
    let mut db = Db::in_memory().unwrap();
    let gh = fake(vec![issue("orbit", 12, "Crash on start")]);
    sync(&mut db, &gh, NOW).unwrap();
    let t = issue_tasks(&db)[0].clone();
    db.update_task(t.id, TaskPatch { title: Some("Fix the crash".into()), priority: Some(Priority::High), status: Some(Status::InProgress), ..Default::default() }, NOW).unwrap();
    sync(&mut db, &gh, NOW + 10).unwrap();
    let t = db.get_task(t.id).unwrap();
    assert_eq!((t.title.as_str(), t.priority, t.status), ("Fix the crash", Priority::High, Status::InProgress));
}

#[test]
fn a_closed_or_unassigned_issue_moves_its_task_to_done() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![issue("orbit", 12, "Crash"), issue("lab", 4, "Backup")]), NOW).unwrap();
    let report = sync(&mut db, &fake(vec![issue("lab", 4, "Backup")]), NOW + 60).unwrap();
    assert_eq!(report.issues_closed, 1);
    let t = issue_tasks(&db).into_iter().find(|t| t.issue_number == Some(12)).unwrap();
    assert_eq!((t.status, t.closed), (Status::Done, Some(NOW + 60)));
    let other = issue_tasks(&db).into_iter().find(|t| t.issue_number == Some(4)).unwrap();
    assert_eq!(other.status, Status::Todo);
}

#[test]
fn a_canceled_issue_task_stays_canceled() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![issue("orbit", 12, "Crash")]), NOW).unwrap();
    let t = issue_tasks(&db)[0].clone();
    db.update_task(t.id, TaskPatch { status: Some(Status::Canceled), ..Default::default() }, NOW).unwrap();
    assert_eq!(sync(&mut db, &fake(vec![]), NOW + 60).unwrap().issues_closed, 0);
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Canceled);
}

#[test]
fn a_deleted_issue_task_stays_deleted() {
    let mut db = Db::in_memory().unwrap();
    let gh = fake(vec![issue("orbit", 12, "Crash")]);
    sync(&mut db, &gh, NOW).unwrap();
    db.delete_task(issue_tasks(&db)[0].id).unwrap();
    assert_eq!(sync(&mut db, &gh, NOW + 60).unwrap().issues_added, 0);
    assert!(issue_tasks(&db).is_empty());
}

#[test]
fn a_failed_issue_read_closes_nothing() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![issue("orbit", 12, "Crash")]), NOW).unwrap();
    let failing = fake(vec![]);
    *failing.issues.borrow_mut() = Err(Error::Invalid("rate limited".into()));
    let report = sync(&mut db, &failing, NOW + 60).unwrap();
    assert_eq!(report.issues_closed, 0);
    assert_eq!(issue_tasks(&db)[0].status, Status::Todo);
    assert_eq!(db.list_projects().unwrap().len(), 2, "repos still synced");
}

#[test]
fn health_sets_pull_requests_and_ci() {
    let mut db = Db::in_memory().unwrap();
    let gh = fake(vec![]);
    *gh.health.borrow_mut() = Ok(vec![
        Health { name_with_owner: "demo-dev/orbit".into(), open_prs: 3, ci: Some(Ci::Failing) },
        Health { name_with_owner: "demo-dev/lab".into(), open_prs: 0, ci: None },
    ]);
    sync(&mut db, &gh, NOW).unwrap();
    let ps = db.list_projects().unwrap();
    let get = |n: &str| ps.iter().find(|p| p.name == n).unwrap().clone();
    assert_eq!((get("orbit").open_prs, get("orbit").ci), (Some(3), Some(Ci::Failing)));
    assert_eq!((get("lab").open_prs, get("lab").ci), (Some(0), None));
}

#[test]
fn rollup_states_map_to_ci() {
    assert_eq!(ci_of(Some("SUCCESS")), Some(Ci::Passing));
    assert_eq!(ci_of(Some("FAILURE")), Some(Ci::Failing));
    assert_eq!(ci_of(Some("ERROR")), Some(Ci::Failing));
    assert_eq!(ci_of(Some("PENDING")), Some(Ci::Running));
    assert_eq!(ci_of(Some("EXPECTED")), Some(Ci::Running));
    assert_eq!(ci_of(None), None);
}

#[test]
fn search_output_parses_into_issues() {
    let out = r#"[{"number":12,"title":"Crash","body":"It crashes","url":"https://github.com/demo-dev/orbit/issues/12","repository":{"name":"orbit","nameWithOwner":"demo-dev/orbit"}}]"#;
    assert_eq!(parse_issues(out).unwrap(), vec![issue("orbit", 12, "Crash")].into_iter().map(|mut i| { i.body = "It crashes".into(); i }).collect::<Vec<_>>());
}

#[test]
fn paginated_graphql_pages_parse_into_health() {
    let page = |name: &str, n: i64, state: &str| {
        format!(r#"{{"data":{{"viewer":{{"repositories":{{"nodes":[{{"nameWithOwner":"demo-dev/{name}","pullRequests":{{"totalCount":{n}}},"defaultBranchRef":{{"target":{{"statusCheckRollup":{state}}}}}}}],"pageInfo":{{"hasNextPage":false,"endCursor":null}}}}}}}}}}"#)
    };
    let out = format!("{}{}", page("orbit", 2, r#"{"state":"SUCCESS"}"#), page("lab", 0, "null"));
    let h = parse_health(&out).unwrap();
    assert_eq!(h, vec![
        Health { name_with_owner: "demo-dev/orbit".into(), open_prs: 2, ci: Some(Ci::Passing) },
        Health { name_with_owner: "demo-dev/lab".into(), open_prs: 0, ci: None },
    ]);
}

#[test]
fn a_task_reopened_after_its_issue_closed_stays_open() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![issue("orbit", 12, "Crash")]), NOW).unwrap();
    assert_eq!(sync(&mut db, &fake(vec![]), NOW + 60).unwrap().issues_closed, 1);
    let t = issue_tasks(&db)[0].clone();
    db.update_task(t.id, TaskPatch { status: Some(Status::InProgress), ..Default::default() }, NOW + 120).unwrap();
    assert_eq!(sync(&mut db, &fake(vec![]), NOW + 180).unwrap().issues_closed, 0);
    assert_eq!(db.get_task(t.id).unwrap().status, Status::InProgress);
    // Reopened and assigned again, it can close again later.
    sync(&mut db, &fake(vec![issue("orbit", 12, "Crash")]), NOW + 240).unwrap();
    assert_eq!(sync(&mut db, &fake(vec![]), NOW + 300).unwrap().issues_closed, 1);
}

#[test]
fn a_task_for_an_issue_never_assigned_to_the_owner_is_left_alone() {
    let mut db = Db::in_memory().unwrap();
    sync(&mut db, &fake(vec![]), NOW).unwrap();
    let orbit = db.list_projects().unwrap().into_iter().find(|p| p.name == "orbit").unwrap();
    // Work on it, on a contributor's issue nobody assigned.
    let t = binder_core::gh_work::issue_task(&mut db, orbit.id, &issue("orbit", 30, "Typo"), NOW).unwrap();
    assert_eq!(sync(&mut db, &fake(vec![]), NOW + 60).unwrap().issues_closed, 0);
    assert_eq!(db.get_task(t.id).unwrap().status, Status::Todo);
}
