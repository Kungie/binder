use binder_core::Error;
use binder_core::cloud::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

fn root() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("Photos/2026")).unwrap();
    fs::write(d.path().join("notes.txt"), "hi").unwrap();
    fs::write(d.path().join(".hidden"), "x").unwrap();
    fs::write(d.path().join("Photos/a.jpg"), "jpg").unwrap();
    d
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.name.as_str()).collect()
}

fn outside(r: Result<std::path::PathBuf, Error>) -> bool {
    matches!(r, Err(Error::Invalid(m)) if m == "That path is outside Cloud.")
}

#[test]
fn resolve_refuses_escapes() {
    let d = root();
    std::os::unix::fs::symlink("/tmp", d.path().join("out")).unwrap();
    assert!(outside(resolve(d.path(), "../x")));
    assert!(outside(resolve(d.path(), "/etc/hosts")));
    assert!(outside(resolve(d.path(), "Photos/../../x")));
    assert!(outside(resolve(d.path(), "out")));
    assert!(outside(resolve(d.path(), "out/new.txt")));
    assert!(resolve(d.path(), "Photos/new.jpg").is_ok(), "a new file inside is fine");
    assert!(resolve(d.path(), "").is_ok());
}

#[test]
fn list_orders_and_hides() {
    let d = root();
    fs::create_dir(d.path().join("archive")).unwrap();
    fs::create_dir(d.path().join(TRASH)).unwrap();
    assert_eq!(names(&list(d.path(), "", false).unwrap()), ["archive", "Photos", "notes.txt"]);
    assert_eq!(names(&list(d.path(), "", true).unwrap()), ["archive", "Photos", ".hidden", "notes.txt"]);
    let photos = list(d.path(), "Photos", false).unwrap();
    assert_eq!((photos[1].rel.as_str(), photos[1].kind, photos[1].size), ("Photos/a.jpg", Kind::File, 3));
    assert_eq!(photos[0].kind, Kind::Dir);
}

#[test]
fn copy_reports_progress_and_finishes() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    let data: Vec<u8> = (0..3 * 1024 * 1024 + 5).map(|i| (i % 251) as u8).collect();
    fs::write(src.path().join("big.bin"), &data).unwrap();
    let mut calls = vec![];
    let out = copy_into(&src.path().join("big.bin"), d.path(), Conflict::KeepBoth, &AtomicBool::new(false), &mut |done, total| calls.push((done, total)))
        .unwrap()
        .unwrap();
    assert!(calls.len() >= 3);
    assert_eq!(calls.last().unwrap(), &(data.len() as u64, data.len() as u64));
    assert_eq!(fs::read(out).unwrap(), data);
}

#[test]
fn cancel_removes_partial_file() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::write(src.path().join("big.bin"), vec![7u8; 3 * 1024 * 1024]).unwrap();
    let cancel = AtomicBool::new(false);
    let r = copy_into(&src.path().join("big.bin"), d.path(), Conflict::KeepBoth, &cancel, &mut |_, _| cancel.store(true, Ordering::SeqCst));
    assert!(matches!(r, Err(Error::Invalid(m)) if m == "Cancelled."));
    let left: Vec<_> = fs::read_dir(d.path()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|n| n.contains("big")).collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn conflicts_replace_keep_both_skip() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::write(src.path().join("a.jpg"), "new").unwrap();
    fs::create_dir(src.path().join("2026")).unwrap();
    let photos = d.path().join("Photos");
    let never = AtomicBool::new(false);
    let kept = copy_into(&src.path().join("a.jpg"), &photos, Conflict::KeepBoth, &never, &mut |_, _| {}).unwrap().unwrap();
    assert_eq!(kept.file_name().unwrap(), "a 2.jpg");
    assert_eq!(copy_into(&src.path().join("a.jpg"), &photos, Conflict::Skip, &never, &mut |_, _| {}).unwrap(), None);
    copy_into(&src.path().join("a.jpg"), &photos, Conflict::Replace, &never, &mut |_, _| {}).unwrap();
    assert_eq!(fs::read_to_string(photos.join("a.jpg")).unwrap(), "new");
    let folder = copy_into(&src.path().join("2026"), &photos, Conflict::KeepBoth, &never, &mut |_, _| {}).unwrap().unwrap();
    assert_eq!(folder.file_name().unwrap(), "2026 2");
    assert_eq!(free_name(&photos, "a.jpg").file_name().unwrap(), "a 3.jpg");
    assert_eq!(free_name(&photos, "README").file_name().unwrap(), "README");
}

#[test]
fn copy_folder_recursively() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::create_dir_all(src.path().join("trip/day1")).unwrap();
    fs::write(src.path().join("trip/day1/x.jpg"), "x").unwrap();
    fs::write(src.path().join("trip/y.jpg"), "yy").unwrap();
    let mut last = (0, 0);
    copy_into(&src.path().join("trip"), d.path(), Conflict::KeepBoth, &AtomicBool::new(false), &mut |a, b| last = (a, b)).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("trip/day1/x.jpg")).unwrap(), "x");
    assert_eq!(last, (3, 3));
}

#[test]
fn trash_then_restore_puts_it_back() {
    let d = root();
    trash(d.path(), "Photos/a.jpg").unwrap();
    assert!(!d.path().join("Photos/a.jpg").exists());
    let items = list_trash(d.path()).unwrap();
    assert_eq!((items.len(), items[0].origin.as_str(), items[0].entry.name.as_str()), (1, "Photos/a.jpg", "a.jpg"));
    assert!(!names(&list(d.path(), "", true).unwrap()).contains(&TRASH), "the trash isn't a folder in the list");
    // Its folder is gone by the time it's restored.
    fs::remove_dir_all(d.path().join("Photos")).unwrap();
    let back = restore(d.path(), &items[0].id).unwrap();
    assert_eq!(back, d.path().join("Photos/a.jpg"));
    assert_eq!(fs::read_to_string(back).unwrap(), "jpg");
    assert!(list_trash(d.path()).unwrap().is_empty());
}

#[test]
fn trash_clash_keeps_both() {
    let d = root();
    trash(d.path(), "notes.txt").unwrap();
    fs::write(d.path().join("notes.txt"), "second").unwrap();
    trash(d.path(), "notes.txt").unwrap();
    let items = list_trash(d.path()).unwrap();
    assert_eq!(items.len(), 2);
    fs::write(d.path().join("notes.txt"), "third").unwrap();
    let back = restore(d.path(), &items[0].id).unwrap();
    assert_eq!(back.file_name().unwrap(), "notes 2.txt");
}

#[test]
fn empty_trash_removes_everything() {
    let d = root();
    trash(d.path(), "Photos").unwrap();
    trash(d.path(), "notes.txt").unwrap();
    empty_trash(d.path()).unwrap();
    assert!(list_trash(d.path()).unwrap().is_empty());
    assert!(matches!(trash(d.path(), ""), Err(Error::Invalid(_))), "the root can't be trashed");
}

#[test]
fn rename_mkdir_and_move() {
    let d = root();
    for bad in ["a/b", ".", "..", "", "  "] {
        assert!(matches!(rename(d.path(), "notes.txt", bad), Err(Error::Invalid(_))), "{bad:?}");
        assert!(matches!(mkdir(d.path(), "", bad), Err(Error::Invalid(_))), "{bad:?}");
    }
    assert!(matches!(rename(d.path(), "notes.txt", "Photos"), Err(Error::Invalid(_))), "a taken name");
    rename(d.path(), "notes.txt", "todo.txt").unwrap();
    mkdir(d.path(), "Photos", "2025").unwrap();
    move_to(d.path(), "todo.txt", "Photos/2025").unwrap();
    assert!(Path::new(&d.path().join("Photos/2025/todo.txt")).exists());
    assert!(matches!(move_to(d.path(), "Photos", "Photos/2025"), Err(Error::Invalid(_))), "a folder into itself");
}

#[test]
fn space_reports_nonzero_total() {
    let d = root();
    let s = space(d.path()).unwrap();
    assert!(s.total > 0 && s.free <= s.total);
}
