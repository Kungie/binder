use binder_core::model::*;
use binder_core::projects::derive_prefix;
use binder_core::{Db, Error};

fn project(db: &mut Db, name: &str) -> Project {
    db.create_project(NewProject { name: name.into(), ..Default::default() }, 1).unwrap()
}

#[test]
fn derive_prefix_takes_three_letters() {
    assert_eq!(derive_prefix("dawn", &[]), "DAW");
    assert_eq!(derive_prefix("Şirket ağı", &[]), "SIR");
}

#[test]
fn derive_prefix_avoids_taken() {
    assert_eq!(derive_prefix("dawn", &["DAW".into()]), "DAW2");
    assert_eq!(derive_prefix("dawn", &["DAW".into(), "DAW2".into()]), "DAW3");
}

#[test]
fn derive_prefix_falls_back() {
    assert_eq!(derive_prefix("7", &[]), "PRJ");
}

#[test]
fn project_tasks_use_its_prefix() {
    let mut db = Db::in_memory().unwrap();
    let p = project(&mut db, "dawn");
    let in_p = |db: &mut Db| db.create_task(NewTask { title: "t".into(), project_id: Some(p.id), ..Default::default() }, 1).unwrap().key;
    assert_eq!(in_p(&mut db), "DAW-1");
    assert_eq!(db.create_task(NewTask { title: "loose".into(), ..Default::default() }, 1).unwrap().key, "TSK-1");
    assert_eq!(in_p(&mut db), "DAW-2");
}

#[test]
fn second_project_with_same_letters_gets_new_prefix() {
    let mut db = Db::in_memory().unwrap();
    project(&mut db, "dawn");
    assert_eq!(project(&mut db, "Dawning").prefix, "DAW2");
}

#[test]
fn given_prefix_is_upper_cased() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "homelab".into(), prefix: Some("lab".into()), ..Default::default() }, 1).unwrap();
    assert_eq!(p.prefix, "LAB");
}

#[test]
fn duplicate_name_is_invalid() {
    let mut db = Db::in_memory().unwrap();
    project(&mut db, "dawn");
    assert!(matches!(db.create_project(NewProject { name: " dawn ".into(), ..Default::default() }, 1), Err(Error::Invalid(_))));
}

#[test]
fn bad_prefix_is_invalid() {
    let mut db = Db::in_memory().unwrap();
    for bad in ["a b", "TOOLONG", "X"] {
        let r = db.create_project(NewProject { name: format!("p {bad}"), prefix: Some(bad.into()), ..Default::default() }, 1);
        assert!(matches!(r, Err(Error::Invalid(_))), "{bad}");
    }
}

#[test]
fn taken_prefix_is_invalid() {
    let mut db = Db::in_memory().unwrap();
    project(&mut db, "dawn");
    let r = db.create_project(NewProject { name: "other".into(), prefix: Some("daw".into()), ..Default::default() }, 1);
    assert!(matches!(r, Err(Error::Invalid(_))));
}

#[test]
fn deleting_project_keeps_its_tasks() {
    let mut db = Db::in_memory().unwrap();
    let p = project(&mut db, "dawn");
    let t = db.create_task(NewTask { title: "t".into(), project_id: Some(p.id), ..Default::default() }, 1).unwrap();
    db.delete_project(p.id).unwrap();
    let after = db.get_task(t.id).unwrap();
    assert_eq!((after.project_id, after.key.as_str()), (None, "DAW-1"));
    assert!(matches!(db.get_project(p.id), Err(Error::NotFound("project", _))));
}

#[test]
fn list_orders_active_first() {
    let mut db = Db::in_memory().unwrap();
    let a = project(&mut db, "zeta");
    let b = project(&mut db, "alpha");
    let c = project(&mut db, "beta");
    db.update_project(b.id, ProjectPatch { state: Some(ProjectState::Done), ..Default::default() }).unwrap();
    db.update_project(c.id, ProjectPatch { state: Some(ProjectState::Paused), ..Default::default() }).unwrap();
    let names: Vec<_> = db.list_projects().unwrap().into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["zeta", "beta", "alpha"]);
    let _ = a;
}

#[test]
fn update_renames_and_clears_repo() {
    let mut db = Db::in_memory().unwrap();
    let p = db.create_project(NewProject { name: "dawn".into(), repo: Some("/r".into()), ..Default::default() }, 1).unwrap();
    let u = db.update_project(p.id, ProjectPatch { name: Some("Dawn".into()), clear_repo: true, ..Default::default() }).unwrap();
    assert_eq!((u.name.as_str(), u.repo, u.prefix.as_str()), ("Dawn", None, "DAW"));
}

#[test]
fn set_note_is_stored() {
    let mut db = Db::in_memory().unwrap();
    let p = project(&mut db, "dawn");
    db.set_project_note(p.id, "/Volumes/vault/Projects/dawn.md").unwrap();
    assert_eq!(db.get_project(p.id).unwrap().note.as_deref(), Some("/Volumes/vault/Projects/dawn.md"));
}

#[test]
fn agent_dir_is_set_and_cleared() {
    let mut db = Db::in_memory().unwrap();
    let p = project(&mut db, "homelab");
    assert_eq!(p.agent_dir, None);
    let p = db.update_project(p.id, ProjectPatch { agent_dir: Some("~/homelab".into()), ..Default::default() }).unwrap();
    assert_eq!(p.agent_dir.as_deref(), Some("~/homelab"));
    let p = db.update_project(p.id, ProjectPatch { agent_dir: Some("  ".into()), ..Default::default() }).unwrap();
    assert_eq!(p.agent_dir, None);
}
