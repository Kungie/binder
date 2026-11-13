use binder_core::github::*;
use binder_core::model::*;
use binder_core::{Db, Error, Result};
use std::cell::RefCell;
use std::os::unix::fs::PermissionsExt;

const NOW: i64 = 1_794_600_000; // 2026-11-13

struct FakeGitHub(RefCell<Vec<Repo>>);

impl GitHub for FakeGitHub {
    fn login(&self) -> Result<String> {
        Ok("demo-dev".into())
    }
    fn repos(&self, login: &str) -> Result<Vec<Repo>> {
        assert_eq!(login, "demo-dev");
        Ok(self.0.borrow().clone())
    }
}

fn repo(name: &str, days_ago: i64) -> Repo {
    let pushed = chrono::DateTime::from_timestamp(NOW - days_ago * 86_400, 0).unwrap().to_rfc3339();
    Repo {
        id: Some(format!("R_{name}")),
        name: name.into(),
        name_with_owner: format!("demo-dev/{name}"),
        description: Some(format!("About {name}")),
        url: format!("https://github.com/demo-dev/{name}"),
        is_private: false,
        is_archived: false,
        pushed_at: pushed,
        language: Some("Rust".into()),
    }
}

fn by_name(db: &Db, name: &str) -> Project {
    db.list_projects().unwrap().into_iter().find(|p| p.name == name).unwrap()
}

#[test]
fn new_repos_become_projects_with_unique_prefixes() {
    let mut db = Db::in_memory().unwrap();
    let names = ["gut", "gut-site", "dawn", "binder", "nashi-app", "nashi-mini", "inert", "zeagen-site", "3-2-1", "doorbell.ai", "chernobyl", "better-termin", "haiku-decides", "bib-sensor-firmware"];
    let gh = FakeGitHub(RefCell::new(names.iter().map(|n| repo(n, 1)).collect()));
    let report = sync(&mut db, &gh, NOW).unwrap();
    assert_eq!(report, SyncReport { added: 14, linked: 0, updated: 0, archived: 0 });
    let projects = db.list_projects().unwrap();
    let prefixes: std::collections::HashSet<_> = projects.iter().map(|p| p.prefix.clone()).collect();
    assert_eq!(prefixes.len(), 14);
    let gut = by_name(&db, "gut");
    assert_eq!(gut.github.as_deref(), Some("demo-dev/gut"));
    assert_eq!(gut.language.as_deref(), Some("Rust"));
    // A second sync with nothing new changes nothing.
    assert_eq!(sync(&mut db, &gh, NOW).unwrap(), SyncReport::default());
}

#[test]
fn recent_push_is_active_old_is_paused() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("fresh", 59), repo("stale", 61)]));
    sync(&mut db, &gh, NOW).unwrap();
    assert_eq!(by_name(&db, "fresh").state, ProjectState::Active);
    assert_eq!(by_name(&db, "stale").state, ProjectState::Paused);
}

#[test]
fn owner_edits_survive_sync() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("dawn", 1)]));
    sync(&mut db, &gh, NOW).unwrap();
    let p = by_name(&db, "dawn");
    let color = binder_core::projects::PALETTE[5].to_string();
    db.update_project(
        p.id,
        ProjectPatch { name: Some("Dawn app".into()), state: Some(ProjectState::Done), color: Some(color.clone()), repo: Some("/code/dawn".into()), ..Default::default() },
    )
    .unwrap();
    gh.0.borrow_mut()[0].description = Some("A menu bar app".into());
    assert_eq!(sync(&mut db, &gh, NOW).unwrap().updated, 1);
    let p = db.get_project(p.id).unwrap();
    assert_eq!((p.name.as_str(), p.state, p.color.as_str(), p.repo.as_deref()), ("Dawn app", ProjectState::Done, color.as_str(), Some("/code/dawn")));
    assert_eq!(p.description.as_deref(), Some("A menu bar app"));
}

#[test]
fn hand_project_with_same_name_is_linked() {
    let mut db = Db::in_memory().unwrap();
    let mine = db.create_project(NewProject { name: "Inert".into(), ..Default::default() }, 1).unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("inert", 1)]));
    assert_eq!(sync(&mut db, &gh, NOW).unwrap(), SyncReport { added: 0, linked: 1, updated: 0, archived: 0 });
    assert_eq!(db.list_projects().unwrap().len(), 1);
    let p = db.get_project(mine.id).unwrap();
    assert_eq!((p.name.as_str(), p.github.as_deref()), ("Inert", Some("demo-dev/inert")));
}

#[test]
fn missing_or_archived_repo_is_marked_not_deleted() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("a", 1), repo("b", 1)]));
    sync(&mut db, &gh, NOW).unwrap();
    *gh.0.borrow_mut() = vec![Repo { is_archived: true, ..repo("a", 1) }];
    assert_eq!(sync(&mut db, &gh, NOW).unwrap().archived, 2);
    assert!(by_name(&db, "a").archived && by_name(&db, "b").archived);
    assert_eq!(db.list_projects().unwrap().len(), 2);
}

#[test]
fn returning_repo_is_unmarked() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("a", 1)]));
    sync(&mut db, &gh, NOW).unwrap();
    gh.0.borrow_mut().clear();
    sync(&mut db, &gh, NOW).unwrap();
    assert!(by_name(&db, "a").archived);
    *gh.0.borrow_mut() = vec![repo("a", 1)];
    sync(&mut db, &gh, NOW).unwrap();
    assert!(!by_name(&db, "a").archived);
}

#[test]
fn hand_projects_without_github_are_never_archived() {
    let mut db = Db::in_memory().unwrap();
    db.create_project(NewProject { name: "workshop".into(), ..Default::default() }, 1).unwrap();
    sync(&mut db, &FakeGitHub(RefCell::new(vec![])), NOW).unwrap();
    assert!(!by_name(&db, "workshop").archived);
}

#[test]
fn gh_auth_error_reads_as_sign_in_line() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("gh");
    std::fs::write(&path, "#!/bin/sh\necho 'To get started with GitHub CLI, please run:  gh auth login' >&2\nexit 4\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = GhCli { program: path };
    match gh.login() {
        Err(Error::Invalid(m)) => assert_eq!(m, "Sign in with gh auth login to bring in your GitHub repos."),
        other => panic!("{other:?}"),
    }
}

#[test]
fn gh_json_parses() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("gh");
    std::fs::write(
        &path,
        "#!/bin/sh\necho '[{\"name\":\"dawn\",\"nameWithOwner\":\"demo-dev/dawn\",\"description\":\"\",\"url\":\"https://github.com/demo-dev/dawn\",\"isPrivate\":true,\"isArchived\":false,\"pushedAt\":\"2026-11-11T10:00:00Z\",\"primaryLanguage\":{\"name\":\"Swift\"}}]'\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let repos = GhCli { program: path }.repos("demo-dev").unwrap();
    assert_eq!(repos[0].language.as_deref(), Some("Swift"));
    assert_eq!(repos[0].description, None, "an empty description is none");
    assert!(repos[0].is_private);
}

#[test]
fn a_renamed_repo_keeps_its_project() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("gut", 1)]));
    sync(&mut db, &gh, NOW).unwrap();
    let before = by_name(&db, "gut");
    *gh.0.borrow_mut() = vec![Repo { id: Some("R_gut".into()), ..repo("gut-cli", 1) }];
    let report = sync(&mut db, &gh, NOW).unwrap();
    assert_eq!(report, SyncReport { updated: 1, ..Default::default() });
    let projects = db.list_projects().unwrap();
    assert_eq!(projects.len(), 1);
    let p = &projects[0];
    assert_eq!((p.id, p.name.as_str(), p.archived), (before.id, "gut", false));
    assert_eq!(p.github.as_deref(), Some("demo-dev/gut-cli"));
    assert_eq!(p.url.as_deref(), Some("https://github.com/demo-dev/gut-cli"));
}

fn owned(name_with_owner: &str, id: Option<&str>) -> Repo {
    let name = name_with_owner.split('/').nth(1).unwrap();
    Repo { id: id.map(String::from), name_with_owner: name_with_owner.into(), url: format!("https://github.com/{name_with_owner}"), ..repo(name, 1) }
}

#[test]
fn deleted_github_project_stays_deleted() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![repo("a", 1), repo("b", 1)]));
    sync(&mut db, &gh, NOW).unwrap();
    db.delete_project(by_name(&db, "a").id, NOW).unwrap();
    let report = sync(&mut db, &gh, NOW).unwrap();
    assert_eq!(report.added, 0);
    assert_eq!(db.list_projects().unwrap().len(), 1);
    let hidden = db.list_hidden_repos().unwrap();
    assert_eq!(hidden.len(), 1);
    assert_eq!((hidden[0].github_id.as_deref(), hidden[0].name_with_owner.as_str(), hidden[0].hidden_at), (Some("R_a"), "demo-dev/a", NOW));
}

#[test]
fn hidden_by_name_when_repo_had_no_id() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![owned("Owner/Repo", None)]));
    sync(&mut db, &gh, NOW).unwrap();
    db.delete_project(by_name(&db, "Repo").id, NOW).unwrap();
    *gh.0.borrow_mut() = vec![owned("owner/repo", None)];
    assert_eq!(sync(&mut db, &gh, NOW).unwrap().added, 0);
    assert!(db.list_projects().unwrap().is_empty());
}

#[test]
fn hidden_repo_is_not_linked_by_name() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![owned("owner/notes", Some("R_notes"))]));
    sync(&mut db, &gh, NOW).unwrap();
    db.delete_project(by_name(&db, "notes").id, NOW).unwrap();
    db.create_project(NewProject { name: "notes".into(), ..Default::default() }, NOW).unwrap();
    sync(&mut db, &gh, NOW).unwrap();
    assert_eq!(by_name(&db, "notes").github, None);
}

#[test]
fn unhide_brings_it_back() {
    let mut db = Db::in_memory().unwrap();
    let gh = FakeGitHub(RefCell::new(vec![owned("owner/notes", Some("R_notes"))]));
    sync(&mut db, &gh, NOW).unwrap();
    db.delete_project(by_name(&db, "notes").id, NOW).unwrap();
    db.unhide_repo("owner/notes").unwrap();
    assert!(db.list_hidden_repos().unwrap().is_empty());
    assert_eq!(sync(&mut db, &gh, NOW).unwrap().added, 1);
}

#[test]
fn deleting_a_hand_added_project_hides_nothing() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "scratch".into(), ..Default::default() }, NOW).unwrap();
    db.delete_project(p.id, NOW).unwrap();
    assert!(db.list_hidden_repos().unwrap().is_empty());
}
