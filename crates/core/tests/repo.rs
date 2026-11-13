use binder_core::Error;
use binder_core::repo::repo_status;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap()
        .status
        .success();
    assert!(ok, "git {args:?}");
}

fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    std::fs::write(d.path().join("a.txt"), "one").unwrap();
    git(d.path(), &["add", "."]);
    git(d.path(), &["commit", "-qm", "First commit"]);
    d
}

#[test]
fn reports_branch_changes_and_last_commit() {
    let d = repo();
    std::fs::write(d.path().join("a.txt"), "two").unwrap();
    std::fs::write(d.path().join("b.txt"), "new").unwrap();
    let s = repo_status(d.path()).unwrap();
    assert_eq!(s.branch.as_deref(), Some("main"));
    assert_eq!(s.changed, 2);
    assert_eq!((s.ahead, s.behind), (0, 0));
    assert_eq!(s.last_subject.as_deref(), Some("First commit"));
    assert_eq!(s.last_date.as_ref().map(|d| d.len()), Some(10));
}

#[test]
fn counts_ahead_of_upstream() {
    let d = repo();
    let bare = tempfile::tempdir().unwrap();
    git(bare.path(), &["init", "-q", "--bare"]);
    git(d.path(), &["remote", "add", "origin", bare.path().to_str().unwrap()]);
    git(d.path(), &["push", "-q", "-u", "origin", "main"]);
    std::fs::write(d.path().join("a.txt"), "two").unwrap();
    git(d.path(), &["commit", "-qam", "Second commit"]);
    let s = repo_status(d.path()).unwrap();
    assert_eq!((s.ahead, s.behind, s.changed), (1, 0, 0));
    assert_eq!(s.last_subject.as_deref(), Some("Second commit"));
}

#[test]
fn not_a_repo_is_invalid() {
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(repo_status(d.path()), Err(Error::Invalid(m)) if m == "Not a git repository"));
}
