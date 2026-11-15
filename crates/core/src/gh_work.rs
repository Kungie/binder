//! The GitHub screen: open pull requests and issues across the owner's repos, one of them in detail, and the
//! owner's own actions on them, all through `gh` and its login on the Mac. Every value goes to `gh` as its own
//! argument, never through a shell.

use crate::github::{GhCli, ci_of};
use crate::model::Ci;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filter {
    NeedsYou,
    Mine,
    AllOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    Approved,
    ChangesRequested,
    ReviewRequired,
    Draft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PullRow {
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub author: String,
    pub ci: Option<Ci>,
    pub review: Option<Review>,
    /// The owner's review is asked for.
    pub review_requested: bool,
    /// The owner opened it.
    pub mine: bool,
    /// RFC 3339.
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueRow {
    pub repo: String,
    pub number: i64,
    pub title: String,
    pub author: String,
    pub assignees: Vec<String>,
    pub updated: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lists {
    pub login: String,
    pub pulls: Vec<PullRow>,
    pub issues: Vec<IssueRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment {
    pub author: String,
    pub body: String,
    pub at: String,
    /// A review's verdict (`APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`); `None` for a plain comment.
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detail {
    pub title: String,
    pub body: String,
    pub head: Option<String>,
    pub base: Option<String>,
    pub additions: i64,
    pub deletions: i64,
    pub files: i64,
    pub checks: Vec<(String, Option<Ci>)>,
    /// The last 20 comments and reviews, oldest first.
    pub comments: Vec<Comment>,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "snake_case")]
pub enum Action {
    Comment(String),
    Approve(String),
    RequestChanges(String),
    Merge,
    Close,
    Reopen,
    AssignMe,
}

const LISTS_QUERY: &str = "query($endCursor: String) { viewer { login repositories(first: 50, after: $endCursor, ownerAffiliations: OWNER) {
  nodes { nameWithOwner
    pullRequests(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { number title isDraft updatedAt author { login } reviewDecision
      reviewRequests(first: 10) { nodes { requestedReviewer { ... on User { login } } } }
      commits(last: 1) { nodes { commit { statusCheckRollup { state } } } } } }
    issues(states: OPEN, first: 50, orderBy: {field: UPDATED_AT, direction: DESC}) { nodes { number title updatedAt author { login } assignees(first: 5) { nodes { login } } } } }
  pageInfo { hasNextPage endCursor } } } }";

fn bad(e: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("gh sent something unexpected: {e}"))
}

fn s(v: &Value, ptr: &str) -> String {
    v.pointer(ptr).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The pages of `LISTS_QUERY`, one JSON object after another.
pub fn parse_lists(out: &str) -> Result<Lists> {
    let mut lists = Lists { login: String::new(), pulls: vec![], issues: vec![] };
    for page in serde_json::Deserializer::from_str(out).into_iter::<Value>() {
        let page = page.map_err(bad)?;
        lists.login = s(&page, "/data/viewer/login");
        let repos = page.pointer("/data/viewer/repositories/nodes").and_then(Value::as_array).cloned().unwrap_or_default();
        for r in repos {
            let repo = s(&r, "/nameWithOwner");
            for p in r.pointer("/pullRequests/nodes").and_then(Value::as_array).cloned().unwrap_or_default() {
                let author = s(&p, "/author/login");
                let review = if p.get("isDraft").and_then(Value::as_bool) == Some(true) {
                    Some(Review::Draft)
                } else {
                    match p.get("reviewDecision").and_then(Value::as_str) {
                        Some("APPROVED") => Some(Review::Approved),
                        Some("CHANGES_REQUESTED") => Some(Review::ChangesRequested),
                        Some("REVIEW_REQUIRED") => Some(Review::ReviewRequired),
                        _ => None,
                    }
                };
                let requested: Vec<String> = p
                    .pointer("/reviewRequests/nodes")
                    .and_then(Value::as_array)
                    .map(|n| n.iter().map(|x| s(x, "/requestedReviewer/login")).collect())
                    .unwrap_or_default();
                let ci = p.pointer("/commits/nodes/0/commit/statusCheckRollup/state").and_then(Value::as_str);
                lists.pulls.push(PullRow {
                    repo: repo.clone(),
                    number: p.get("number").and_then(Value::as_i64).unwrap_or(0),
                    title: s(&p, "/title"),
                    review_requested: !lists.login.is_empty() && requested.contains(&lists.login),
                    mine: !lists.login.is_empty() && author == lists.login,
                    author,
                    ci: ci_of(ci),
                    review,
                    updated: s(&p, "/updatedAt"),
                });
            }
            for i in r.pointer("/issues/nodes").and_then(Value::as_array).cloned().unwrap_or_default() {
                lists.issues.push(IssueRow {
                    repo: repo.clone(),
                    number: i.get("number").and_then(Value::as_i64).unwrap_or(0),
                    title: s(&i, "/title"),
                    author: s(&i, "/author/login"),
                    assignees: i.pointer("/assignees/nodes").and_then(Value::as_array).map(|n| n.iter().map(|x| s(x, "/login")).collect()).unwrap_or_default(),
                    updated: s(&i, "/updatedAt"),
                });
            }
        }
    }
    lists.pulls.sort_by(|a, b| b.updated.cmp(&a.updated));
    lists.issues.sort_by(|a, b| b.updated.cmp(&a.updated));
    Ok(lists)
}

/// Review asked of the owner, or the owner's own with failing checks or changes asked; issues assigned to the owner.
pub fn needs_you(pulls: &[PullRow], issues: &[IssueRow], login: &str) -> (Vec<PullRow>, Vec<IssueRow>) {
    let p = pulls
        .iter()
        .filter(|p| p.review_requested || (p.mine && (p.ci == Some(Ci::Failing) || p.review == Some(Review::ChangesRequested))))
        .cloned()
        .collect();
    let i = issues.iter().filter(|i| i.assignees.iter().any(|a| a == login)).cloned().collect();
    (p, i)
}

/// The owner's pull requests, and issues assigned to or opened by the owner.
pub fn mine(pulls: &[PullRow], issues: &[IssueRow], login: &str) -> (Vec<PullRow>, Vec<IssueRow>) {
    let p = pulls.iter().filter(|p| p.mine).cloned().collect();
    let i = issues.iter().filter(|i| i.author == login || i.assignees.iter().any(|a| a == login)).cloned().collect();
    (p, i)
}

fn check(c: &Value) -> (String, Option<Ci>) {
    let name = c.get("name").or_else(|| c.get("context")).and_then(Value::as_str).unwrap_or_default().to_string();
    let state = match (c.get("status").and_then(Value::as_str), c.get("conclusion").and_then(Value::as_str), c.get("state").and_then(Value::as_str)) {
        (Some("COMPLETED"), Some(conclusion), _) => match conclusion {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => Some(Ci::Passing),
            _ => Some(Ci::Failing),
        },
        (Some(_), _, _) => Some(Ci::Running),
        (None, _, Some(state)) => ci_of(Some(state)),
        _ => None,
    };
    (name, state)
}

/// `gh pr view --json …` or `gh issue view --json …`.
pub fn parse_detail(out: &str) -> Result<Detail> {
    let v: Value = serde_json::from_str(out).map_err(bad)?;
    let mut comments: Vec<Comment> = v
        .get("comments")
        .and_then(Value::as_array)
        .map(|c| c.iter().map(|x| Comment { author: s(x, "/author/login"), body: s(x, "/body"), at: s(x, "/createdAt"), state: None }).collect())
        .unwrap_or_default();
    if let Some(reviews) = v.get("reviews").and_then(Value::as_array) {
        comments.extend(reviews.iter().map(|x| Comment { author: s(x, "/author/login"), body: s(x, "/body"), at: s(x, "/submittedAt"), state: Some(s(x, "/state")) }));
    }
    comments.sort_by(|a, b| a.at.cmp(&b.at));
    let keep = comments.len().saturating_sub(20);
    comments.drain(..keep);
    let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(Detail {
        title: s(&v, "/title"),
        body: s(&v, "/body"),
        head: text("headRefName"),
        base: text("baseRefName"),
        additions: n("additions"),
        deletions: n("deletions"),
        files: n("changedFiles"),
        checks: v.get("statusCheckRollup").and_then(Value::as_array).map(|c| c.iter().map(check).collect()).unwrap_or_default(),
        comments,
        url: s(&v, "/url"),
    })
}

/// Squash when the repo allows it, then a merge commit, then rebase.
pub fn merge_flag(squash: bool, merge: bool, _rebase: bool) -> &'static str {
    if squash {
        "--squash"
    } else if merge {
        "--merge"
    } else {
        "--rebase"
    }
}

/// The `gh` arguments for an action; a merge gets its method added by `act`.
pub fn args(repo: &str, number: i64, pull: bool, action: &Action) -> Vec<String> {
    let kind = if pull { "pr" } else { "issue" };
    let n = number.to_string();
    let base = |verb: &str| vec![kind.to_string(), verb.to_string(), n.clone(), "-R".to_string(), repo.to_string()];
    match action {
        Action::Comment(body) => [base("comment"), vec!["--body".into(), body.clone()]].concat(),
        Action::Approve(body) if body.trim().is_empty() => [base("review"), vec!["--approve".into()]].concat(),
        Action::Approve(body) => [base("review"), vec!["--approve".into(), "--body".into(), body.clone()]].concat(),
        Action::RequestChanges(body) => [base("review"), vec!["--request-changes".into(), "--body".into(), body.clone()]].concat(),
        Action::Merge => base("merge"),
        Action::Close => base("close"),
        Action::Reopen => base("reopen"),
        Action::AssignMe => [base("edit"), vec!["--add-assignee".into(), "@me".into()]].concat(),
    }
}

/// The number at the end of the url `gh issue create` prints.
pub fn issue_number_of(out: &str) -> Option<i64> {
    out.trim().rsplit('/').next()?.parse().ok()
}

pub trait GhWork {
    fn lists(&self) -> Result<Lists>;
    fn detail(&self, repo: &str, number: i64, pull: bool) -> Result<Detail>;
    fn diff(&self, repo: &str, number: i64) -> Result<String>;
    fn act(&self, repo: &str, number: i64, pull: bool, action: &Action) -> Result<()>;
    fn new_issue(&self, repo: &str, title: &str, body: &str) -> Result<i64>;
}

impl GhWork for GhCli {
    fn lists(&self) -> Result<Lists> {
        parse_lists(&self.run(&["api", "graphql", "--paginate", "-f", &format!("query={LISTS_QUERY}")])?)
    }

    fn detail(&self, repo: &str, number: i64, pull: bool) -> Result<Detail> {
        let n = number.to_string();
        let out = if pull {
            self.run(&["pr", "view", &n, "-R", repo, "--json", "title,body,headRefName,baseRefName,additions,deletions,changedFiles,statusCheckRollup,comments,reviews,url"])?
        } else {
            self.run(&["issue", "view", &n, "-R", repo, "--json", "title,body,comments,url"])?
        };
        parse_detail(&out)
    }

    fn diff(&self, repo: &str, number: i64) -> Result<String> {
        self.run(&["pr", "diff", &number.to_string(), "-R", repo])
    }

    fn act(&self, repo: &str, number: i64, pull: bool, action: &Action) -> Result<()> {
        let mut a = args(repo, number, pull, action);
        if *action == Action::Merge {
            let v: Value = serde_json::from_str(&self.run(&["repo", "view", repo, "--json", "squashMergeAllowed,mergeCommitAllowed,rebaseMergeAllowed"])?).map_err(bad)?;
            let allowed = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
            a.push(merge_flag(allowed("squashMergeAllowed"), allowed("mergeCommitAllowed"), allowed("rebaseMergeAllowed")).into());
        }
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        self.run(&refs)?;
        Ok(())
    }

    fn new_issue(&self, repo: &str, title: &str, body: &str) -> Result<i64> {
        let out = self.run(&["issue", "create", "-R", repo, "--title", title, "--body", body])?;
        issue_number_of(&out).ok_or_else(|| bad("no issue number"))
    }
}

/// Diffs longer than this are cut before they reach the agent.
const DIFF_LIMIT: usize = 200_000;

/// What the agent reads to review a pull request. It answers in its output; Binder shows that as a draft.
pub fn review_prompt(title: &str, body: &str, diff: &str) -> String {
    let (diff, cut) = if diff.len() > DIFF_LIMIT {
        let mut end = DIFF_LIMIT;
        while !diff.is_char_boundary(end) {
            end -= 1;
        }
        (&diff[..end], format!("\n\nThe diff was cut at {DIFF_LIMIT} characters."))
    } else {
        (diff, String::new())
    };
    format!(
        "Review this pull request. Write a short review in Markdown: what it does in two or three sentences, the risks, \
         then concrete fixes with the file and line. Plain words, no praise. Don't post anything and don't change any \
         files: your answer is a draft the owner reads first.\n\nTitle: {title}\n\n{body}\n\n```diff\n{diff}\n```{cut}"
    )
}

/// What the agent is asked when the owner hands it an issue.
pub fn work_prompt(key: &str, repo: &str, number: i64, title: &str, body: &str) -> String {
    let branch = format!("binder/{}", key.to_lowercase());
    format!(
        "Work on issue #{number} of {repo}: {title}\n\n{body}\n\nIn this folder, make a branch {branch} from the default \
         branch, fix the issue with small commits, run the tests, then `git push -u origin {branch}` and open a draft pull \
         request with `gh pr create --draft --repo {repo}` whose body says `Closes #{number}`. Don't merge anything."
    )
}

/// What the agent is asked to fix a pull request's failing checks or requested changes.
pub fn fix_prompt(repo: &str, number: i64, branch: &str) -> String {
    format!(
        "Fix pull request #{number} of {repo} on its branch {branch}. Run `gh pr checkout {number} --repo {repo}`, read the \
         failed checks with `gh run list --repo {repo} --branch {branch}` and `gh run view <id> --repo {repo} --log-failed`, \
         and the review comments with `gh pr view {number} --repo {repo} --comments`. Fix them with small commits, run the \
         tests and push to the same branch. Don't merge or close anything."
    )
}

/// A task for the agent's review of a pull request, in progress until the run ends.
pub fn review_task(db: &mut crate::Db, project_id: i64, repo: &str, number: i64, title: &str, now: i64) -> Result<crate::model::Task> {
    use crate::model::{Assignee, NewTask, Status};
    let _ = repo;
    db.create_task(
        NewTask { title: format!("Review #{number}: {title}"), project_id: Some(project_id), assignee: Some(Assignee::Agent), status: Some(Status::InProgress), ..Default::default() },
        now,
    )
}

/// The issue's task, made if sync hasn't brought it in, and given to the agent.
pub fn issue_task(db: &mut crate::Db, project_id: i64, issue: &crate::github::Issue, now: i64) -> Result<crate::model::Task> {
    use crate::model::{Assignee, NewTask, TaskPatch};
    let id = match db.issue_task_id(&issue.repo, issue.number)? {
        Some(id) => id,
        None => {
            let t = db.create_task(NewTask { title: issue.title.clone(), notes: Some(issue.body.clone()), project_id: Some(project_id), ..Default::default() }, now)?;
            db.link_issue(t.id, &issue.repo, issue.number, &issue.url)?;
            t.id
        }
    };
    db.update_task(id, TaskPatch { assignee: Some(Assignee::Agent), ..Default::default() }, now)
}
