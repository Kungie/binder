use binder_core::gh_work::*;
use binder_core::model::Ci;

const LISTS: &str = r#"{"data":{"viewer":{"login":"demo-dev","repositories":{"nodes":[
 {"nameWithOwner":"demo-dev/orbit",
  "pullRequests":{"nodes":[
   {"number":41,"title":"Sync status","isDraft":false,"updatedAt":"2026-11-15T08:00:00Z","author":{"login":"friend"},"reviewDecision":"REVIEW_REQUIRED",
    "reviewRequests":{"nodes":[{"requestedReviewer":{"login":"demo-dev"}}]},"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"SUCCESS"}}}]}},
   {"number":40,"title":"My change","isDraft":false,"updatedAt":"2026-11-14T08:00:00Z","author":{"login":"demo-dev"},"reviewDecision":"CHANGES_REQUESTED",
    "reviewRequests":{"nodes":[]},"commits":{"nodes":[{"commit":{"statusCheckRollup":null}}]}},
   {"number":39,"title":"Wip","isDraft":true,"updatedAt":"2026-11-13T08:00:00Z","author":{"login":"demo-dev"},"reviewDecision":null,
    "reviewRequests":{"nodes":[]},"commits":{"nodes":[{"commit":{"statusCheckRollup":{"state":"FAILURE"}}}]}},
   {"number":38,"title":"Someone else's","isDraft":false,"updatedAt":"2026-11-12T08:00:00Z","author":{"login":"friend"},"reviewDecision":"APPROVED",
    "reviewRequests":{"nodes":[]},"commits":{"nodes":[]}}]},
  "issues":{"nodes":[
   {"number":12,"title":"Crash","updatedAt":"2026-11-15T07:00:00Z","author":{"login":"friend"},"assignees":{"nodes":[{"login":"demo-dev"}]}},
   {"number":11,"title":"Idea","updatedAt":"2026-11-10T07:00:00Z","author":{"login":"demo-dev"},"assignees":{"nodes":[]}}]}}],
 "pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#;

#[test]
fn pulls_parse_ci_and_review_state() {
    let l = parse_lists(LISTS).unwrap();
    assert_eq!(l.login, "demo-dev");
    assert_eq!(l.pulls.len(), 4);
    let p = &l.pulls[0];
    assert_eq!((p.repo.as_str(), p.number, p.author.as_str(), p.ci, p.review, p.review_requested, p.mine), ("demo-dev/orbit", 41, "friend", Some(Ci::Passing), Some(Review::ReviewRequired), true, false));
    assert_eq!((l.pulls[1].review, l.pulls[1].ci, l.pulls[1].mine), (Some(Review::ChangesRequested), None, true));
    assert_eq!((l.pulls[2].review, l.pulls[2].ci), (Some(Review::Draft), Some(Ci::Failing)));
    assert_eq!(l.pulls[3].review, Some(Review::Approved));
    assert_eq!(l.issues[0].assignees, vec!["demo-dev".to_string()]);
}

#[test]
fn needs_you_picks_requests_failing_and_assigned() {
    let l = parse_lists(LISTS).unwrap();
    let (pulls, issues) = needs_you(&l.pulls, &l.issues, &l.login);
    assert_eq!(pulls.iter().map(|p| p.number).collect::<Vec<_>>(), vec![41, 40, 39]);
    assert_eq!(issues.iter().map(|i| i.number).collect::<Vec<_>>(), vec![12]);
    let (mine_p, mine_i) = mine(&l.pulls, &l.issues, &l.login);
    assert_eq!(mine_p.iter().map(|p| p.number).collect::<Vec<_>>(), vec![40, 39]);
    assert_eq!(mine_i.iter().map(|i| i.number).collect::<Vec<_>>(), vec![12, 11]);
}

#[test]
fn detail_keeps_the_last_20_comments() {
    let comments: Vec<String> = (0..25).map(|i| format!(r#"{{"author":{{"login":"a{i}"}},"body":"c{i}","createdAt":"2026-11-15T0{}:00:00Z"}}"#, i % 10)).collect();
    let json = format!(
        r#"{{"title":"Sync status","body":"About it","headRefName":"feature","baseRefName":"main","additions":120,"deletions":34,"changedFiles":6,"url":"https://github.com/demo-dev/orbit/pull/41",
        "statusCheckRollup":[{{"__typename":"CheckRun","name":"test","status":"COMPLETED","conclusion":"SUCCESS"}},{{"__typename":"StatusContext","context":"lint","state":"PENDING"}}],
        "comments":[{}],"reviews":[{{"author":{{"login":"friend"}},"body":"Looks fine","state":"APPROVED","submittedAt":"2026-11-15T09:30:00Z"}}]}}"#,
        comments.join(",")
    );
    let d = parse_detail(&json).unwrap();
    assert_eq!(d.title, "Sync status");
    assert_eq!((d.head.as_deref(), d.base.as_deref(), d.additions, d.deletions, d.files), (Some("feature"), Some("main"), 120, 34, 6));
    assert_eq!(d.checks, vec![("test".to_string(), Some(Ci::Passing)), ("lint".to_string(), Some(Ci::Running))]);
    assert_eq!(d.comments.len(), 20);
    assert_eq!(d.comments.last().unwrap().state.as_deref(), Some("APPROVED"), "the newest is the review at 09:30");
    let issue = parse_detail(r#"{"body":"Steps","url":"https://github.com/demo-dev/orbit/issues/12","comments":[]}"#).unwrap();
    assert_eq!((issue.head, issue.files, issue.comments.len()), (None, 0, 0));
}

#[test]
fn merge_picks_squash_when_allowed() {
    assert_eq!(merge_flag(true, true, true), "--squash");
    assert_eq!(merge_flag(false, true, true), "--merge");
    assert_eq!(merge_flag(false, false, true), "--rebase");
}

#[test]
fn a_comment_reaches_gh_as_one_argument() {
    let body = r#"Fine "but" $(rm -rf ~) `x`; and 'this'"#;
    let a = args("demo-dev/orbit", 41, true, &Action::Comment(body.into()));
    assert_eq!(a, vec!["pr", "comment", "41", "-R", "demo-dev/orbit", "--body", body]);
    assert_eq!(args("demo-dev/orbit", 12, false, &Action::Close), vec!["issue", "close", "12", "-R", "demo-dev/orbit"]);
    assert_eq!(args("demo-dev/orbit", 41, true, &Action::Approve(String::new())), vec!["pr", "review", "41", "-R", "demo-dev/orbit", "--approve"]);
    assert_eq!(args("demo-dev/orbit", 41, true, &Action::RequestChanges("Fix x".into())), vec!["pr", "review", "41", "-R", "demo-dev/orbit", "--request-changes", "--body", "Fix x"]);
    assert_eq!(args("demo-dev/orbit", 12, false, &Action::AssignMe), vec!["issue", "edit", "12", "-R", "demo-dev/orbit", "--add-assignee", "@me"]);
    assert_eq!(args("demo-dev/orbit", 41, true, &Action::Reopen), vec!["pr", "reopen", "41", "-R", "demo-dev/orbit"]);
}

#[test]
fn a_new_issue_url_gives_its_number() {
    assert_eq!(issue_number_of("https://github.com/demo-dev/orbit/issues/57\n"), Some(57));
    assert_eq!(issue_number_of("nothing"), None);
}

#[test]
fn review_prompt_cuts_a_huge_diff() {
    let p = review_prompt("Sync status", "Adds a dot", "x".repeat(250_000).as_str());
    assert!(p.contains("Sync status") && p.contains("Adds a dot"));
    assert!(p.len() < 205_000, "{}", p.len());
    assert!(p.contains("The diff was cut at 200000 characters."));
    assert!(p.contains("Don't post anything"));
    let short = review_prompt("t", "b", "diff --git a/x b/x");
    assert!(!short.contains("was cut"));
}

#[test]
fn work_prompt_names_the_branch_and_closes_the_issue() {
    let p = work_prompt("ORB-7", "demo-dev/orbit", 12, "Crash", "Steps");
    assert!(p.contains("binder/orb-7") && p.contains("Closes #12") && p.contains("gh pr create --draft") && p.contains("demo-dev/orbit"));
    let f = fix_prompt("demo-dev/orbit", 41, "feature");
    assert!(f.contains("gh pr checkout 41") && f.contains("gh run view") && f.contains("--log-failed") && f.contains("feature"));
}

#[test]
fn issue_task_reuses_the_synced_task() {
    let mut db = binder_core::Db::in_memory().unwrap();
    let p = db.create_project(binder_core::model::NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    let issue = binder_core::github::Issue { repo: "demo-dev/orbit".into(), number: 12, title: "Crash".into(), body: "Steps".into(), url: "https://github.com/demo-dev/orbit/issues/12".into() };
    let a = issue_task(&mut db, p.id, &issue, 10).unwrap();
    assert_eq!((a.assignee, a.issue_number), (binder_core::model::Assignee::Agent, Some(12)));
    let b = issue_task(&mut db, p.id, &issue, 20).unwrap();
    assert_eq!(a.id, b.id);
    let r = review_task(&mut db, p.id, "demo-dev/orbit", 41, "Sync status", 30).unwrap();
    assert_eq!((r.title.as_str(), r.assignee, r.status), ("Review #41: Sync status", binder_core::model::Assignee::Agent, binder_core::model::Status::InProgress));
}

#[test]
fn a_failed_start_gives_the_task_back() {
    use binder_core::model::{Assignee, Status};
    let mut db = binder_core::Db::in_memory().unwrap();
    let p = db.create_project(binder_core::model::NewProject { name: "orbit".into(), ..Default::default() }, 1).unwrap();
    // A task made only for the run is canceled, so pick-up never runs its bare title.
    let r = review_task(&mut db, p.id, "demo-dev/orbit", 41, "Sync status", 30).unwrap();
    let r = undo_start(&mut db, r.id, true, 40).unwrap();
    assert_eq!((r.assignee, r.status), (Assignee::Me, Status::Canceled));
    // The issue's own task goes back to the owner and stays open.
    let issue = binder_core::github::Issue { repo: "demo-dev/orbit".into(), number: 12, title: "Crash".into(), body: "Steps".into(), url: "https://github.com/demo-dev/orbit/issues/12".into() };
    let t = issue_task(&mut db, p.id, &issue, 50).unwrap();
    let t = undo_start(&mut db, t.id, false, 60).unwrap();
    assert_eq!((t.assignee, t.status), (Assignee::Me, Status::Todo));
}
