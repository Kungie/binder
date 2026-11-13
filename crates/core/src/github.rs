//! The owner's GitHub repos as projects, through the `gh` CLI and its own login. Binder never sees a token.

use crate::model::{NewProject, ProjectState};
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

pub trait GitHub {
    fn login(&self) -> Result<String>;
    fn repos(&self, login: &str) -> Result<Vec<Repo>>;
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

    fn run(&self, args: &[&str]) -> Result<String> {
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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SyncReport {
    pub added: usize,
    pub linked: usize,
    pub updated: usize,
    pub archived: usize,
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
    db.conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = apply(db, &repos, now);
    match result {
        Ok(report) => {
            db.conn.execute_batch("COMMIT")?;
            Ok(report)
        }
        Err(e) => {
            let _ = db.conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
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
    for repo in repos {
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
