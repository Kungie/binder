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
    assert_eq!(note_path(v, "a/b: c#d"), Path::new("/v/Projects/a-b- c-d.md"));
    assert_eq!(note_path(v, "Şirket ağı"), Path::new("/v/Projects/Şirket ağı.md"));
    assert_eq!(note_path(v, " [] "), Path::new("/v/Projects/Untitled.md"));
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
            DoneLine { date: "2026-11-12".into(), text: "one".into() },
            DoneLine { date: "2026-11-13".into(), text: "two · with dot".into() },
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

#[test]
fn done_on_filters_by_day() {
    let dir = tempfile::tempdir().unwrap();
    let path = note_in(dir.path(), "## Done\n- 2026-11-12 · old\n- 2026-11-13 · today\n");
    let projects = [project("dawn", None, Some(&path)), project("no note", None, None), project("gone", None, Some(&dir.path().join("x.md")))];
    let got = done_on(&projects, "2026-11-13");
    assert_eq!(got, vec![("dawn".to_string(), DoneLine { date: "2026-11-13".into(), text: "today".into() })]);
}
