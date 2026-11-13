//! A project's local git state: branch, uncommitted changes, distance from upstream, last commit.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStatus {
    pub branch: Option<String>,
    pub changed: u32,
    pub ahead: u32,
    pub behind: u32,
    pub last_subject: Option<String>,
    pub last_date: Option<String>,
}

fn git(path: &Path, args: &[&str]) -> Result<Option<String>> {
    let out = Command::new("git").arg("-C").arg(path).args(args).output()?;
    Ok(out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned()))
}

pub fn repo_status(path: &Path) -> Result<RepoStatus> {
    let status = git(path, &["status", "--porcelain=v2", "--branch"])?
        .ok_or_else(|| Error::Invalid("Not a git repository".into()))?;
    let mut s = RepoStatus::default();
    for line in status.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            s.branch = (head != "(detached)").then(|| head.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for w in ab.split_whitespace() {
                if let Some(n) = w.strip_prefix('+') {
                    s.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = w.strip_prefix('-') {
                    s.behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            s.changed += 1;
        }
    }
    // An empty repo has no commit yet: git log fails and the last commit stays empty.
    if let Some(log) = git(path, &["log", "-1", "--format=%s%x00%cs"])? {
        if let Some((subject, date)) = log.trim_end().split_once('\0') {
            s.last_subject = Some(subject.to_string());
            s.last_date = Some(date.to_string());
        }
    }
    Ok(s)
}
