use binder_core::model::*;
use binder_core::notes::*;
use binder_core::Error;
use std::fs;
use std::path::Path;

fn project(name: &str, repo: Option<&str>, note: Option<&Path>) -> Project {
    Project {
        agent_dir: None,
        github: None,
        url: None,
        description: None,
        pushed_at: None,
        private: false,
        archived: false,
        color: "#5E6AD2".into(),
        language: None,
        id: 1,
        name: name.into(),
        prefix: "P".into(),
        repo: repo.map(Into::into),
        note: note.map(|p| p.to_string_lossy().into_owned()),
        state: ProjectState::Active,
        created: 0,
    }
}

fn note_in(dir: &Path, text: &str) -> std::path::PathBuf {
    fs::create_dir_all(dir.join("Projects")).unwrap();
    let path = dir.join("Projects/n.md");
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn note_path_is_safe() {
    let v = Path::new("/v");
    assert_eq!(note_path(v, "a/b: c#d"), Path::new("/v/Projects/a-b- c-d/a-b- c-d.md"));
    assert_eq!(note_path(v, "Şirket ağı"), Path::new("/v/Projects/Şirket ağı/Şirket ağı.md"));
    assert_eq!(note_path(v, " [] "), Path::new("/v/Projects/Untitled/Untitled.md"));
    assert_eq!(legacy_note_path(v, "a/b: c#d"), Path::new("/v/Projects/a-b- c-d.md"));
}

#[test]
fn ensure_note_writes_template_once() {
    let dir = tempfile::tempdir().unwrap();
    let (path, created) = ensure_note(dir.path(), &project("dawn", Some("/src/dawn"), None), "2026-11-13").unwrap();
    assert!(created);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "---\nproject: dawn\nrepo: /src/dawn\nstarted: 2026-11-13\n---\n\n# dawn\n\n## Done\n"
    );
    fs::write(&path, "mine").unwrap();
    let (again, created) = ensure_note(dir.path(), &project("dawn", None, None), "2026-11-14").unwrap();
    assert_eq!((again, created), (path.clone(), false));
    assert_eq!(fs::read_to_string(&path).unwrap(), "mine");
}

#[test]
fn ensure_note_needs_vault() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("vault");
    assert!(matches!(ensure_note(&missing, &project("dawn", None, None), "2026-11-13"), Err(Error::VaultMissing(_))));
    assert!(!missing.exists());
}

#[test]
fn append_into_existing_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "# x\n\n## Done\n- 2026-11-12 · old\n\n## Ideas\nkeep me\n");
    assert!(append_done(&path, "2026-11-13", "new").unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "# x\n\n## Done\n- 2026-11-12 · old\n- 2026-11-13 · new\n\n## Ideas\nkeep me\n");
}

#[test]
fn append_into_empty_section_at_end() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "# x\n\n## Done\n");
    append_done(&path, "2026-11-13", "first").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# x\n\n## Done\n- 2026-11-13 · first\n");
}

#[test]
fn append_adds_section_when_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "# x\nbody");
    append_done(&path, "2026-11-13", "a").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# x\nbody\n\n## Done\n- 2026-11-13 · a\n");
}

#[test]
fn append_keeps_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "# x\r\n\r\n## Done\r\n- 2026-11-12 · old\r\n\r\n## Ideas\r\n");
    append_done(&path, "2026-11-13", "new").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "# x\r\n\r\n## Done\r\n- 2026-11-12 · old\r\n- 2026-11-13 · new\r\n\r\n## Ideas\r\n");
}

#[test]
fn append_skips_empty_and_flattens_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "## Done\n");
    assert!(!append_done(&path, "2026-11-13", "  \n ").unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "## Done\n");
    append_done(&path, "2026-11-13", "a\nb").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "## Done\n- 2026-11-13 · a b\n");
}

#[test]
fn append_needs_vault() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault/Projects/n.md");
    assert!(matches!(append_done(&path, "2026-11-13", "a"), Err(Error::VaultMissing(_))));
    assert!(!dir.path().join("vault").exists());
}

#[test]
fn one_line_caps_length() {
    let long = "ş".repeat(250);
    assert_eq!(one_line(&long).unwrap().chars().count(), 200);
    assert_eq!(one_line("  a \t b\n"), Some("a b".into()));
    assert_eq!(one_line(" "), None);
}

#[test]
fn read_done_parses_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "## Done\n- 2026-11-12 · one\nnot a line\n- 2026-11-13 · two · with dot\n## Next\n- 2026-11-14 · elsewhere\n");
    let lines = read_done(&path).unwrap();
    assert_eq!(
        lines,
        vec![
            line("2026-11-12", None, "one"),
            line("2026-11-13", None, "two · with dot"),
        ]
    );
    assert!(read_done(&dir.path().join("none.md")).unwrap().is_empty());
}

#[test]
fn obsidian_url_encodes_utf8() {
    assert_eq!(
        obsidian_url(Path::new("/Volumes/vault/Projects/Şirket ağı.md")),
        "obsidian://open?path=/Volumes/vault/Projects/%C5%9Eirket%20a%C4%9F%C4%B1.md"
    );
}

fn line(date: &str, time: Option<&str>, text: &str) -> DoneLine {
    DoneLine { date: date.into(), time: time.map(Into::into), text: text.into() }
}

#[test]
fn note_moves_into_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = legacy_note_path(dir.path(), "dawn");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let bytes = "---\nproject: dawn\n---\n\n# dawn\nmine\r\n\n## Done\n- 2026-11-01 · old\n";
    fs::write(&legacy, bytes).unwrap();
    let (path, made) = ensure_note(dir.path(), &project("dawn", None, None), "2026-11-13").unwrap();
    assert!(!made);
    assert_eq!(path, dir.path().join("Projects/dawn/dawn.md"));
    assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    assert!(!legacy.exists());
}

#[test]
fn note_move_skipped_when_new_path_taken() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = legacy_note_path(dir.path(), "dawn");
    let new = note_path(dir.path(), "dawn");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::write(&legacy, "old").unwrap();
    fs::write(&new, "new").unwrap();
    let (path, made) = ensure_note(dir.path(), &project("dawn", None, None), "2026-11-13").unwrap();
    assert_eq!((path.clone(), made), (new, false));
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(fs::read_to_string(&legacy).unwrap(), "old");
}

fn folder_note(vault: &Path, name: &str) -> std::path::PathBuf {
    ensure_note(vault, &project(name, None, None), "2026-11-13").unwrap().0
}

#[test]
fn daily_file_header_and_lines() {
    let dir = tempfile::tempdir().unwrap();
    let note = folder_note(dir.path(), "a/b");
    assert!(append_daily(&note, "a/b", "2026-11-13", Some("14:32"), "Decided X.").unwrap());
    let daily = daily_path(&note, "2026-11-13");
    assert_eq!(daily, dir.path().join("Projects/a-b/Daily/2026-11-13.md"));
    assert_eq!(
        fs::read_to_string(&daily).unwrap(),
        "---\nproject: \"[[a-b]]\"\ndate: 2026-11-13\n---\n\n# a/b, 13 November 2026\n\n- 14:32 · Decided X.\n"
    );
    assert!(!append_daily(&note, "a/b", "2026-11-13", None, " \n ").unwrap());
    append_daily(&note, "a/b", "2026-11-13", None, "No time.").unwrap();
    assert!(fs::read_to_string(&daily).unwrap().ends_with("- 14:32 · Decided X.\n- No time.\n"));
    assert_eq!(read_daily(&note, "2026-11-13").unwrap(), vec![line("2026-11-13", Some("14:32"), "Decided X."), line("2026-11-13", None, "No time.")]);
}

#[test]
fn append_daily_appends_without_touching_earlier_lines() {
    let dir = tempfile::tempdir().unwrap();
    let note = folder_note(dir.path(), "dawn");
    append_daily(&note, "dawn", "2026-11-05", Some("09:00"), "First.").unwrap();
    let daily = daily_path(&note, "2026-11-05");
    let edited = fs::read_to_string(&daily).unwrap().replace("# dawn, 5 November 2026", "# my own title\nsome words");
    fs::write(&daily, &edited).unwrap();
    append_daily(&note, "dawn", "2026-11-05", Some("10:15"), "Second.").unwrap();
    assert_eq!(fs::read_to_string(&daily).unwrap(), format!("{edited}- 10:15 · Second.\n"));
}

#[test]
fn append_daily_needs_vault() {
    let dir = tempfile::tempdir().unwrap();
    let note = dir.path().join("gone/Projects/dawn/dawn.md");
    assert!(matches!(append_daily(&note, "dawn", "2026-11-13", None, "a"), Err(Error::VaultMissing(_))));
}

#[test]
fn read_history_order() {
    let dir = tempfile::tempdir().unwrap();
    let note = folder_note(dir.path(), "dawn");
    fs::write(&note, "# dawn\n\n## Done\n- 2026-10-01 · legacy one\n- 2026-10-02 · legacy two\n").unwrap();
    append_daily(&note, "dawn", "2026-11-12", Some("09:00"), "a").unwrap();
    append_daily(&note, "dawn", "2026-11-13", Some("08:00"), "b").unwrap();
    append_daily(&note, "dawn", "2026-11-13", Some("17:00"), "c").unwrap();
    fs::write(note.parent().unwrap().join("Daily/notes.txt"), "- not a day").unwrap();
    assert_eq!(
        read_history(&note).unwrap(),
        vec![
            line("2026-11-13", Some("17:00"), "c"),
            line("2026-11-13", Some("08:00"), "b"),
            line("2026-11-12", Some("09:00"), "a"),
            line("2026-10-02", None, "legacy two"),
            line("2026-10-01", None, "legacy one"),
        ]
    );
    assert!(read_history(&dir.path().join("none/none.md")).unwrap().is_empty());
}

#[test]
fn done_on_reads_today_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let a = folder_note(dir.path(), "dawn");
    let b = folder_note(dir.path(), "orbit");
    append_daily(&a, "dawn", "2026-11-13", Some("09:00"), "early").unwrap();
    append_daily(&b, "orbit", "2026-11-13", Some("15:30"), "late").unwrap();
    append_daily(&a, "dawn", "2026-11-12", Some("23:00"), "yesterday").unwrap();
    let mut orbit = project("orbit", None, Some(&b));
    orbit.id = 2;
    orbit.color = "#4CB782".into();
    let projects = [project("dawn", None, Some(&a)), orbit, project("no note", None, None), project("gone", None, Some(&dir.path().join("x.md")))];
    let got = done_on(&projects, "2026-11-13");
    assert_eq!(
        got,
        vec![
            DoneItem { project_id: 2, project: "orbit".into(), color: "#4CB782".into(), line: line("2026-11-13", Some("15:30"), "late") },
            DoneItem { project_id: 1, project: "dawn".into(), color: "#5E6AD2".into(), line: line("2026-11-13", Some("09:00"), "early") },
        ]
    );
}

#[test]
fn done_on_filters_by_day() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "## Done\n- 2026-11-12 · old\n- 2026-11-13 · today\n");
    let projects = [project("dawn", None, Some(&path)), project("no note", None, None), project("gone", None, Some(&dir.path().join("x.md")))];
    let got = done_on(&projects, "2026-11-13");
    assert_eq!(got, vec![DoneItem { project_id: 1, project: "dawn".into(), color: "#5E6AD2".into(), line: line("2026-11-13", None, "today") }]);
}
