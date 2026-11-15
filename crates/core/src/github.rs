//! The owner's GitHub repos as projects, through the `gh` CLI and its own login. Binder never sees a token.

use crate::events::EventKind;
use crate::model::{Ci, NewProject, NewTask, ProjectState, Status, TaskPatch};
use crate::{Db, Error, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// A repo is active when it was pushed within this many days.
pub const ACTIVE_DAYS: i64 = 60;
const SIGN_IN: &str = "Sign in with gh auth login to bring in your GitHub repos.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Repo {
    /// GitHub's own id for the repo. It survives renames and transfers, unlike the name.
    pub id: Option<String>,
    pub name: String,
    pub name_with_owner: String,
    pub description: Option<String>,
    pub url: String,
    pub is_private: bool,
    pub is_archived: bool,
    /// RFC 3339.
    pub pushed_at: String,
    pub language: Option<String>,
}

/// `gh repo list --json …` as it comes.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepo {
    #[serde(default)]
    id: Option<String>,
    name: String,
    name_with_owner: String,
    #[serde(default)]
    description: Option<String>,
    url: String,
    #[serde(default)]
    is_private: bool,
    #[serde(default)]
    is_archived: bool,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    primary_language: Option<Language>,
}

#[derive(Deserialize)]
struct Language {
    name: String,
}

impl From<RawRepo> for Repo {
    fn from(r: RawRepo) -> Repo {
        Repo {
            id: r.id.filter(|i| !i.is_empty()),
            name: r.name,
            name_with_owner: r.name_with_owner,
            description: r.description.filter(|d| !d.trim().is_empty()),
            url: r.url,
            is_private: r.is_private,
            is_archived: r.is_archived,
            pushed_at: r.pushed_at.unwrap_or_default(),
            language: r.primary_language.map(|l| l.name).filter(|l| !l.is_empty()),
        }
    }
}

/// An open issue assigned to the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// `owner/name`.
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub body: String,
    pub url: String,
}

/// A repo's open pull requests and the checks on its default branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Health {
    pub name_with_owner: String,
    pub open_prs: i64,
    pub ci: Option<Ci>,
}

/// GitHub's status rollup state as Binder's CI result.
pub fn ci_of(state: Option<&str>) -> Option<Ci> {
    match state? {
        "SUCCESS" => Some(Ci::Passing),
        "FAILURE" | "ERROR" => Some(Ci::Failing),
        "PENDING" | "EXPECTED" => Some(Ci::Running),
        _ => None,
    }
}

pub trait GitHub {
    fn login(&self) -> Result<String>;
    fn repos(&self, login: &str) -> Result<Vec<Repo>>;
    /// Open issues assigned to the owner. An error leaves every issue task as it is.
    fn issues(&self) -> Result<Vec<Issue>> {
        Err(Error::Invalid("This GitHub source has no issues.".into()))
    }
    fn health(&self) -> Result<Vec<Health>> {
        Err(Error::Invalid("This GitHub source has no pull requests or checks.".into()))
    }
}

const HEALTH_QUERY: &str = "query($endCursor: String) { viewer { repositories(first: 100, after: $endCursor, ownerAffiliations: OWNER) {
  nodes { nameWithOwner pullRequests(states: OPEN) { totalCount } defaultBranchRef { target { ... on Commit { statusCheckRollup { state } } } } }
  pageInfo { hasNextPage endCursor } } } }";

/// `gh search issues --json …` as it comes.
pub fn parse_issues(out: &str) -> Result<Vec<Issue>> {
    let raw: Vec<serde_json::Value> = serde_json::from_str(out).map_err(|e| Error::Invalid(format!("gh sent something unexpected: {e}")))?;
    Ok(raw
        .iter()
        .filter_map(|v| {
            let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
            let repo = v.get("repository")?.get("nameWithOwner")?.as_str()?.to_string();
            Some(Issue { repo, number: v.get("number")?.as_i64()?, title: s("title"), body: s("body"), url: s("url") })
        })
        .collect())
}

/// The pages `gh api graphql --paginate` prints, one JSON object after another.
pub fn parse_health(out: &str) -> Result<Vec<Health>> {
    let mut all = vec![];
    for page in serde_json::Deserializer::from_str(out).into_iter::<serde_json::Value>() {
        let page = page.map_err(|e| Error::Invalid(format!("gh sent something unexpected: {e}")))?;
        let nodes = page.pointer("/data/viewer/repositories/nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default();
        for n in nodes {
            let Some(name) = n.get("nameWithOwner").and_then(|x| x.as_str()) else { continue };
            all.push(Health {
                name_with_owner: name.to_string(),
                open_prs: n.pointer("/pullRequests/totalCount").and_then(|x| x.as_i64()).unwrap_or(0),
                ci: ci_of(n.pointer("/defaultBranchRef/target/statusCheckRollup/state").and_then(|x| x.as_str())),
            });
        }
    }
    Ok(all)
}

pub struct GhCli {
    pub program: PathBuf,
}

impl GhCli {
    /// `gh` on the PATH, else where Homebrew puts it. An app opened from Finder has a short PATH.
    pub fn find() -> Option<GhCli> {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")])
            .map(|d| d.join("gh"))
            .find(|p| p.is_file())
            .map(|program| GhCli { program })
    }

    pub(crate) fn run(&self, args: &[&str]) -> Result<String> {
        let out = Command::new(&self.program).args(args).output().map_err(|e| Error::Invalid(format!("Couldn't run gh: {e}")))?;
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("auth login") || err.to_lowercase().contains("not logged") {
            Err(Error::Invalid(SIGN_IN.into()))
        } else {
            Err(Error::Invalid(err.trim().chars().take(300).collect()))
        }
    }
}

impl GitHub for GhCli {
    fn login(&self) -> Result<String> {
        Ok(self.run(&["api", "user", "--jq", ".login"])?.trim().to_string())
    }

    fn repos(&self, login: &str) -> Result<Vec<Repo>> {
        let out = self.run(&[
            "repo",
            "list",
            login,
            "--limit",
            "200",
            "--json",
            "id,name,nameWithOwner,description,url,isPrivate,isArchived,pushedAt,primaryLanguage",
        ])?;
        let raw: Vec<RawRepo> = serde_json::from_str(&out).map_err(|e| Error::Invalid(format!("gh sent something unexpected: {e}")))?;
        Ok(raw.into_iter().map(Repo::from).collect())
    }

    fn issues(&self) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["search", "issues", "--assignee", "@me", "--state", "open", "--limit", "200", "--json", "number,title,body,url,repository"])?)
    }

    fn health(&self) -> Result<Vec<Health>> {
        parse_health(&self.run(&["api", "graphql", "--paginate", "-f", &format!("query={HEALTH_QUERY}")])?)
    }
}

/// A repo the owner deleted as a project. Sync leaves it out until it's brought back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HiddenRepo {
    pub github_id: Option<String>,
    pub name_with_owner: String,
    pub hidden_at: i64,
}

impl Db {
    pub fn list_hidden_repos(&self) -> Result<Vec<HiddenRepo>> {
        let mut st = self.conn.prepare("SELECT github_id, name_with_owner, hidden_at FROM hidden_repos ORDER BY hidden_at DESC, name_with_owner")?;
        let rows = st.query_map([], |r| Ok(HiddenRepo { github_id: r.get(0)?, name_with_owner: r.get(1)?, hidden_at: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Lets sync bring the repo back as a new project.
    pub fn unhide_repo(&mut self, name_with_owner: &str) -> Result<()> {
        self.conn.execute("DELETE FROM hidden_repos WHERE lower(name_with_owner) = lower(?1)", [name_with_owner])?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    pub added: usize,
    pub linked: usize,
    pub updated: usize,
    pub archived: usize,
    pub issues_added: usize,
    pub issues_closed: usize,
}

fn pushed_days_ago(pushed_at: &str, now: i64) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(pushed_at).ok()?.timestamp();
    Some((now - t) / 86_400)
}

/// Brings the owner's repos in as projects. Only GitHub's own facts are ever updated; the owner's edits stay.
/// Projects are never deleted: a repo that is archived or gone marks its project archived.
pub fn sync(db: &mut Db, gh: &dyn GitHub, now: i64) -> Result<SyncReport> {
    let login = gh.login()?;
    let repos = gh.repos(&login)?;
    // Each read may fail on its own; a failed one leaves its part as it was.
    let issues = gh.issues().ok();
    let health = gh.health().ok();
    db.conn.execute_batch("BEGIN IMMEDIATE")?;
    let mut report = match apply(db, &repos, now).and_then(|r| health.as_deref().map_or(Ok(()), |h| apply_health(db, h)).map(|_| r)) {
        Ok(report) => {
            db.conn.execute_batch("COMMIT")?;
            report
        }
        Err(e) => {
            let _ = db.conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    };
    if let Some(issues) = &issues {
        let (added, closed) = apply_issues(db, issues, now)?;
        report.issues_added = added;
        report.issues_closed = closed;
    }
    Ok(report)
}

fn apply_health(db: &mut Db, health: &[Health]) -> Result<()> {
    for h in health {
        db.conn.execute(
            "UPDATE projects SET open_prs = ?2, ci = ?3 WHERE lower(github) = lower(?1)",
            params![h.name_with_owner, h.open_prs, h.ci.map(Ci::as_str)],
        )?;
    }
    Ok(())
}

/// Issues assigned to the owner become tasks once. A task whose issue leaves the list (closed, or no longer the
/// owner's) moves to Done once; reopened by the owner, it stays as they left it. A task linked to an issue that was
/// never on the list (Work on it, on someone else's issue) is never closed by sync. Nothing else is ever changed.
fn apply_issues(db: &mut Db, issues: &[Issue], now: i64) -> Result<(usize, usize)> {
    let projects = db.list_projects()?;
    let mut added = 0;
    for issue in issues {
        let Some(project) = projects.iter().find(|p| p.github.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(&issue.repo))) else { continue };
        let listed = db.conn.execute(
            "UPDATE tasks SET issue_listed = 1 WHERE lower(issue_repo) = lower(?1) AND issue_number = ?2",
            params![issue.repo, issue.number],
        )?;
        if listed > 0 || db.issue_hidden(&issue.repo, issue.number)? {
            continue;
        }
        let task = db.create_task(
            NewTask { title: issue.title.clone(), notes: Some(issue.body.clone()), project_id: Some(project.id), ..Default::default() },
            now,
        )?;
        db.link_issue(task.id, &issue.repo, issue.number, &issue.url)?;
        db.conn.execute("UPDATE tasks SET issue_listed = 1 WHERE id = ?1", [task.id])?;
        db.log_event(now, EventKind::IssueAdded, &task.key, &task.title)?;
        added += 1;
    }
    let gone: Vec<(i64, String, i64, String)> = db
        .conn
        .prepare("SELECT id, issue_repo, issue_number, status FROM tasks WHERE issue_repo IS NOT NULL AND issue_listed = 1")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<Vec<(i64, String, i64, String)>>>()?
        .into_iter()
        .filter(|(_, repo, number, _)| !issues.iter().any(|i| i.number == *number && i.repo.eq_ignore_ascii_case(repo)))
        .collect();
    let mut closed = 0;
    for (id, _, _, status) in gone {
        db.conn.execute("UPDATE tasks SET issue_listed = 0 WHERE id = ?1", [id])?;
        if status != "done" && status != "canceled" {
            db.update_task(id, TaskPatch { status: Some(Status::Done), ..Default::default() }, now)?;
            closed += 1;
        }
    }
    Ok((added, closed))
}

fn apply(db: &mut Db, repos: &[Repo], now: i64) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let projects = db.list_projects()?;
    // GitHub ids per project, so a renamed or transferred repo finds the project it already has.
    let ids: Vec<(i64, String)> = db
        .conn
        .prepare("SELECT id, github_id FROM projects WHERE github_id IS NOT NULL")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let id_of = |p: &crate::model::Project| ids.iter().find(|(pid, _)| *pid == p.id).map(|(_, g)| g.as_str());
    let hidden = db.list_hidden_repos()?;
    let is_hidden = |r: &Repo| {
        hidden.iter().any(|h| {
            (h.github_id.is_some() && h.github_id == r.id) || h.name_with_owner.to_lowercase() == r.name_with_owner.to_lowercase()
        })
    };
    for repo in repos {
        if is_hidden(repo) {
            continue;
        }
        let facts = (&repo.url, &repo.description, &repo.pushed_at, repo.is_private, repo.is_archived, &repo.language);
        let known = repo
            .id
            .as_deref()
            .and_then(|gid| projects.iter().find(|p| id_of(p) == Some(gid)))
            .or_else(|| projects.iter().find(|p| p.github.as_deref() == Some(repo.name_with_owner.as_str())));
        let id = match known {
            Some(p) => {
                let same = (p.github.as_deref(), p.url.as_ref(), p.description.as_ref(), p.pushed_at.as_ref(), p.private, p.archived, p.language.as_ref())
                    == (Some(repo.name_with_owner.as_str()), Some(facts.0), facts.1.as_ref(), Some(facts.2), facts.3, facts.4, facts.5.as_ref());
                if !same {
                    if repo.is_archived && !p.archived {
                        report.archived += 1;
                    } else {
                        report.updated += 1;
                    }
                }
                p.id
            }
            None => match projects.iter().find(|p| p.github.is_none() && p.name.to_lowercase() == repo.name.to_lowercase()) {
                Some(p) => {
                    report.linked += 1;
                    p.id
                }
                None => {
                    let Ok(p) = db.create_project(NewProject { name: repo.name.clone(), ..Default::default() }, now) else {
                        // A name Binder can't take (it would share a note file with another project): skip this repo.
                        continue;
                    };
                    let fresh = pushed_days_ago(&repo.pushed_at, now).is_some_and(|d| d <= ACTIVE_DAYS);
                    let state = if fresh { ProjectState::Active } else { ProjectState::Paused };
                    db.conn.execute("UPDATE projects SET state = ?2 WHERE id = ?1", params![p.id, state.as_str()])?;
                    report.added += 1;
                    p.id
                }
            },
        };
        db.conn.execute(
            "UPDATE projects SET github = ?2, url = ?3, description = ?4, pushed_at = ?5, private = ?6, archived = ?7, language = ?8,
             github_id = COALESCE(?9, github_id) WHERE id = ?1",
            params![id, repo.name_with_owner, repo.url, repo.description, repo.pushed_at, repo.is_private, repo.is_archived, repo.language, repo.id],
        )?;
    }
    for p in projects.iter().filter(|p| !p.archived) {
        if let Some(gh) = &p.github {
            let here = repos.iter().any(|r| &r.name_with_owner == gh || (r.id.is_some() && r.id.as_deref() == id_of(p)));
            if !here {
                db.conn.execute("UPDATE projects SET archived = 1 WHERE id = ?1", [p.id])?;
                report.archived += 1;
            }
        }
    }
    Ok(report)
}
