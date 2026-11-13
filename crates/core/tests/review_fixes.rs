use binder_core::model::*;
use binder_core::{Db, Error};

fn project(db: &mut Db, name: &str) -> binder_core::Result<Project> {
    db.create_project(NewProject { name: name.into(), ..Default::default() }, 1)
}

fn task_in(db: &mut Db, pid: i64) -> binder_core::Result<Task> {
    db.create_task(NewTask { title: "t".into(), project_id: Some(pid), ..Default::default() }, 1)
}

#[test]
fn recreated_project_keeps_unique_keys() {
    let mut db = Db::in_memory().unwrap();
    let p = project(&mut db, "dawn").unwrap();
    assert_eq!(task_in(&mut db, p.id).unwrap().key, "DAW-1");
    db.delete_project(p.id, 0).unwrap();
    let again = project(&mut db, "dawn").unwrap();
    assert_eq!(again.prefix, "DAW");
    assert_eq!(task_in(&mut db, again.id).unwrap().key, "DAW-2");
}

#[test]
fn tsk_prefix_is_reserved() {
    let mut db = Db::in_memory().unwrap();
    let given = db.create_project(NewProject { name: "x".into(), prefix: Some("tsk".into()), ..Default::default() }, 1);
    assert!(matches!(given, Err(Error::Invalid(_))));
    let derived = project(&mut db, "Tskeleton").unwrap();
    assert_ne!(derived.prefix, "TSK");
    db.create_task(NewTask { title: "loose".into(), ..Default::default() }, 1).unwrap();
    task_in(&mut db, derived.id).unwrap();
}

#[test]
fn names_differing_only_by_case_are_one_project() {
    let mut db = Db::in_memory().unwrap();
    project(&mut db, "Şirket").unwrap();
    assert!(matches!(project(&mut db, "şirket"), Err(Error::Invalid(_))));
}

#[test]
fn projects_sharing_a_note_file_are_refused() {
    let mut db = Db::in_memory().unwrap();
    project(&mut db, "A/B").unwrap();
    assert!(matches!(project(&mut db, "A:B"), Err(Error::Invalid(_))));
    let c = project(&mut db, "C").unwrap();
    let renamed = db.update_project(c.id, ProjectPatch { name: Some("a#b".into()), ..Default::default() });
    assert!(matches!(renamed, Err(Error::Invalid(_))));
}
