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
    let out = copy_into(&src.path().join("big.bin"), d.path(), Conflict::KeepBoth, None, &AtomicBool::new(false), &mut |done, total| calls.push((done, total)))
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
    let r = copy_into(&src.path().join("big.bin"), d.path(), Conflict::KeepBoth, None, &cancel, &mut |_, _| cancel.store(true, Ordering::SeqCst));
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
    let kept = copy_into(&src.path().join("a.jpg"), &photos, Conflict::KeepBoth, None, &never, &mut |_, _| {}).unwrap().unwrap();
    assert_eq!(kept.file_name().unwrap(), "a 2.jpg");
    assert_eq!(copy_into(&src.path().join("a.jpg"), &photos, Conflict::Skip, None, &never, &mut |_, _| {}).unwrap(), None);
    copy_into(&src.path().join("a.jpg"), &photos, Conflict::Replace, Some(d.path()), &never, &mut |_, _| {}).unwrap();
    assert_eq!(fs::read_to_string(photos.join("a.jpg")).unwrap(), "new");
    let folder = copy_into(&src.path().join("2026"), &photos, Conflict::KeepBoth, None, &never, &mut |_, _| {}).unwrap().unwrap();
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
    copy_into(&src.path().join("trip"), d.path(), Conflict::KeepBoth, None, &AtomicBool::new(false), &mut |a, b| last = (a, b)).unwrap();
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

#[test]
fn replace_moves_the_old_item_to_trash() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::write(src.path().join("a.jpg"), "new").unwrap();
    let never = AtomicBool::new(false);
    copy_into(&src.path().join("a.jpg"), &d.path().join("Photos"), Conflict::Replace, Some(d.path()), &never, &mut |_, _| {}).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Photos/a.jpg")).unwrap(), "new");
    let items = list_trash(d.path()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].origin, "Photos/a.jpg");
    restore(d.path(), &items[0].id).unwrap();
    assert_eq!(fs::read_to_string(d.path().join("Photos/a 2.jpg")).unwrap(), "jpg", "the old one comes back beside the new one");
}

#[test]
fn replace_outside_cloud_is_refused() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::write(src.path().join("notes.txt"), "new").unwrap();
    let r = copy_into(&src.path().join("notes.txt"), d.path(), Conflict::Replace, None, &AtomicBool::new(false), &mut |_, _| {});
    assert!(matches!(r, Err(Error::Invalid(_))));
    assert_eq!(fs::read_to_string(d.path().join("notes.txt")).unwrap(), "hi");
}

#[test]
fn a_name_taken_during_the_copy_is_kept_both() {
    let d = root();
    let src = tempfile::tempdir().unwrap();
    fs::write(src.path().join("clip.mov"), vec![1u8; 3 * 1024 * 1024]).unwrap();
    let target = d.path().join("clip.mov");
    // Another copy, or the phone, writes the same name while this one runs.
    let landed = copy_into(&src.path().join("clip.mov"), d.path(), Conflict::KeepBoth, None, &AtomicBool::new(false), &mut |_, _| {
        if !target.exists() {
            fs::write(&target, "other").unwrap();
        }
    })
    .unwrap()
    .unwrap();
    assert_eq!(landed.file_name().unwrap(), "clip 2.mov");
    assert_eq!(fs::read_to_string(&target).unwrap(), "other");
    assert_eq!(fs::metadata(&landed).unwrap().len(), 3 * 1024 * 1024);
}

fn partials(dir: &Path) -> Vec<String> {
    fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with(".binder-partial-")).collect()
}

#[test]
fn duplicate_names_like_finder() {
    let d = root();
    let r = d.path();
    fs::write(r.join("a.txt"), "a").unwrap();
    assert_eq!(copy_name(r, "a.txt"), r.join("a copy.txt"));
    let never = AtomicBool::new(false);
    let first = copy_into_as(&r.join("a.txt"), r, "a copy.txt", Conflict::KeepBoth, Some(r), &never, &mut |_, _| {}).unwrap().unwrap();
    assert_eq!(first, r.join("a copy.txt"));
    assert_eq!(fs::read_to_string(&first).unwrap(), "a");
    assert_eq!(copy_name(r, "a.txt"), r.join("a copy 2.txt"));
    assert_eq!(copy_name(r, "Photos"), r.join("Photos copy"));
    fs::create_dir(r.join("v1.2")).unwrap();
    assert_eq!(copy_name(r, "v1.2"), r.join("v1.2 copy"), "a folder never splits at its dot");
    assert_eq!(copy_name(r, "Makefile"), r.join("Makefile copy"));
}

#[test]
fn info_has_created_and_mac_path() {
    let d = root();
    let i = info(d.path(), "Photos/a.jpg").unwrap();
    assert_eq!((i.entry.name.as_str(), i.entry.kind, i.entry.size), ("a.jpg", Kind::File, 3));
    assert_eq!(i.path, d.path().join("Photos/a.jpg").to_string_lossy());
    assert!(i.created > 1_600_000_000, "{}", i.created);
    assert!(info(d.path(), "../x").is_err());
}

#[test]
fn tree_size_counts_bytes_and_items() {
    let d = root();
    fs::write(d.path().join("Photos/2026/b.jpg"), "12345").unwrap();
    std::os::unix::fs::symlink("/etc", d.path().join("Photos/link")).unwrap();
    // a.jpg, 2026, 2026/b.jpg and the link; the link isn't followed.
    assert_eq!(tree_size(d.path(), "Photos").unwrap(), (8 + "/etc".len() as u64, 4));
    assert_eq!(tree_size(d.path(), "notes.txt").unwrap(), (2, 1));
}

#[test]
fn compress_one_and_many() {
    let d = root();
    let never = AtomicBool::new(false);
    let mut seen = vec![];
    let one = compress_local(d.path(), &["notes.txt".into()], &never, &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(one, d.path().join("notes.txt.zip"));
    assert_eq!(seen.last(), Some(&(1, 1)));
    let many = || compress_local(d.path(), &["notes.txt".into(), "Photos".into()], &never, &mut |_, _| {}).unwrap();
    assert_eq!(many(), d.path().join("Archive.zip"));
    assert_eq!(many(), d.path().join("Archive 2.zip"));
    assert!(partials(d.path()).is_empty());
    let mixed = compress_local(d.path(), &["notes.txt".into(), "Photos/a.jpg".into()], &never, &mut |_, _| {});
    assert!(matches!(mixed, Err(Error::Invalid(_))), "items from two folders");
}

#[test]
fn extract_round_trip() {
    let d = root();
    let r = d.path();
    fs::create_dir_all(r.join("Proj/sub")).unwrap();
    fs::create_dir_all(r.join("Proj/empty")).unwrap();
    fs::write(r.join("Proj/a.txt"), "alpha").unwrap();
    let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    fs::write(r.join("Proj/sub/b.bin"), &bytes).unwrap();
    std::os::unix::fs::symlink("a.txt", r.join("Proj/link")).unwrap();
    let never = AtomicBool::new(false);
    let zip = compress_local(r, &["Proj".into()], &never, &mut |_, _| {}).unwrap();
    let mut seen = vec![];
    let out = extract_local(r, "Proj.zip", &never, &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(zip, r.join("Proj.zip"));
    assert_eq!(out, r.join("Proj 2"), "Proj is taken, so a free name");
    let back = out.join("Proj");
    assert_eq!(fs::read_to_string(back.join("a.txt")).unwrap(), "alpha");
    assert_eq!(fs::read(back.join("sub/b.bin")).unwrap(), bytes);
    assert!(back.join("empty").is_dir());
    assert!(fs::symlink_metadata(back.join("link")).unwrap().file_type().is_symlink(), "the link stays a link");
    assert_eq!(fs::read_link(back.join("link")).unwrap(), Path::new("a.txt"));
    let (done, total) = *seen.last().unwrap();
    assert_eq!(done, total);
    assert!(partials(r).is_empty());
}

fn evil_zip(path: &Path, name: &str) {
    let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
    w.start_file("fine.txt", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut w, b"ok").unwrap();
    w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut w, b"evil").unwrap();
    w.finish().unwrap();
}

#[test]
fn extract_refuses_dotdot_entries() {
    let d = root();
    let cloud = d.path().join("cloud");
    fs::create_dir(&cloud).unwrap();
    let never = AtomicBool::new(false);
    for (zip, name) in [("a.zip", "../evil"), ("b.zip", "/evil"), ("c.zip", "x/../../evil")] {
        evil_zip(&cloud.join(zip), name);
        let r = extract_local(&cloud, zip, &never, &mut |_, _| {});
        assert!(matches!(&r, Err(Error::Invalid(m)) if m == "This zip has paths that leave its folder."), "{zip}: {r:?}");
    }
    assert!(!d.path().join("evil").exists());
    assert!(!cloud.join("a").exists());
    assert!(partials(&cloud).is_empty());
}

#[test]
fn extract_refuses_writing_through_a_link() {
    let d = root();
    let cloud = d.path().join("cloud");
    fs::create_dir(&cloud).unwrap();
    let mut w = zip::ZipWriter::new(fs::File::create(cloud.join("l.zip")).unwrap());
    w.add_symlink("up", "..", zip::write::SimpleFileOptions::default()).unwrap();
    w.start_file("up/escaped.txt", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut w, b"x").unwrap();
    w.finish().unwrap();
    let never = AtomicBool::new(false);
    assert!(extract_local(&cloud, "l.zip", &never, &mut |_, _| {}).is_err());
    assert!(!cloud.join("escaped.txt").exists());
    assert!(partials(&cloud).is_empty());
}

#[test]
fn cancel_leaves_no_partial() {
    let d = root();
    let never = AtomicBool::new(false);
    compress_local(d.path(), &["Photos".into()], &never, &mut |_, _| {}).unwrap();
    let cancel = AtomicBool::new(true);
    assert!(compress_local(d.path(), &["notes.txt".into()], &cancel, &mut |_, _| {}).is_err());
    assert!(!d.path().join("notes.txt.zip").exists());
    assert!(extract_local(d.path(), "Photos.zip", &cancel, &mut |_, _| {}).is_err());
    assert!(!d.path().join("Photos 2").exists());
    assert!(partials(d.path()).is_empty());
}

#[test]
fn sweep_moves_dead_partials_to_trash_keeps_live() {
    let d = root();
    fs::write(d.path().join(".binder-partial-111-0-report.pdf"), "half").unwrap();
    fs::create_dir(d.path().join(".binder-partial-222-1-live")).unwrap();
    fs::write(d.path().join("Photos/.binder-partial-111-2-deep.jpg"), "x").unwrap();
    assert_eq!(sweep_partials(d.path(), "", &|pid| pid == 222).unwrap(), 1);
    assert!(d.path().join(".binder-partial-222-1-live").exists());
    assert!(d.path().join("Photos/.binder-partial-111-2-deep.jpg").exists(), "only the given folder");
    let t = list_trash(d.path()).unwrap();
    assert_eq!(t.len(), 1);
    assert!(t[0].unfinished);
    assert_eq!(t[0].entry.name, "report.pdf");
    assert_eq!(sweep_partials(d.path(), "", &|_| false).unwrap(), 1);
}

#[test]
fn unfinished_upload_cannot_be_restored() {
    let d = root();
    fs::write(d.path().join(".binder-partial-111-0-report.pdf"), "half").unwrap();
    sweep_partials(d.path(), "", &|_| false).unwrap();
    let id = list_trash(d.path()).unwrap()[0].id.clone();
    assert!(matches!(restore(d.path(), &id), Err(Error::Invalid(m)) if m == "An unfinished upload can't be restored."));
    let tid = trash(d.path(), "notes.txt").unwrap();
    assert!(!list_trash(d.path()).unwrap().iter().find(|t| t.id == tid).unwrap().unfinished);
}

#[test]
fn land_waits_for_a_partial_the_share_hasnt_shown_yet() {
    let d = root();
    let dir = d.path().to_path_buf();
    let partial = dir.join(partial_name("late.txt"));
    let writer = {
        let partial = partial.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            fs::write(partial, "late").unwrap();
        })
    };
    let landed = land(&partial, &dir, "late.txt", Conflict::KeepBoth, None).unwrap();
    writer.join().unwrap();
    assert_eq!(landed, Some(dir.join("late.txt")));
    assert_eq!(fs::read_to_string(dir.join("late.txt")).unwrap(), "late");
    let never = dir.join(partial_name("never.txt"));
    assert!(land(&never, &dir, "never.txt", Conflict::KeepBoth, None).is_err());
}

/// Folders that stand for other disks: a rename between two disks fails with EXDEV, as the share reports it.
struct FakeDisks(Vec<std::path::PathBuf>);

impl FakeDisks {
    fn disk(&self, p: &Path) -> Option<usize> {
        self.0.iter().enumerate().filter(|(_, d)| p.starts_with(d)).max_by_key(|(_, d)| d.components().count()).map(|(i, _)| i)
    }
}

impl Disks for FakeDisks {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        if self.disk(from) != self.disk(to) {
            return Err(std::io::Error::from_raw_os_error(libc::EXDEV));
        }
        fs::rename(from, to)
    }
}

fn two_disks() -> (tempfile::TempDir, FakeDisks) {
    let d = root();
    fs::create_dir_all(d.path().join("SD/films")).unwrap();
    fs::write(d.path().join("SD/films/a.mkv"), "film").unwrap();
    let disks = FakeDisks(vec![d.path().join("SD")]);
    (d, disks)
}

fn all_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn trash_goes_to_the_items_own_disk() {
    let (d, disks) = two_disks();
    let r = d.path();
    let id = trash_on(&disks, r, "SD/films/a.mkv").unwrap();
    assert!(id.starts_with("SD/"), "{id}");
    assert!(!r.join("SD/films/a.mkv").exists());
    assert_eq!(fs::read_dir(r.join("SD/.trash")).unwrap().filter(|e| e.as_ref().unwrap().file_name() != ".disks").count(), 1);
    assert_eq!(fs::read_to_string(r.join(".trash/.disks")).unwrap(), "SD\n");
    assert!(!r.join("SD/films/.trash").exists(), "no stray trash further down");
    let main_slots: Vec<String> = all_names(&r.join(".trash")).into_iter().filter(|n| n != ".disks").collect();
    assert!(main_slots.is_empty(), "the failed try left a slot: {main_slots:?}");
    // A second item on the card adds no second line.
    fs::write(r.join("SD/b.txt"), "b").unwrap();
    trash_on(&disks, r, "SD/b.txt").unwrap();
    assert_eq!(fs::read_to_string(r.join(".trash/.disks")).unwrap(), "SD\n");
    // The root disk keeps its own trash and plain ids.
    let plain = trash_on(&disks, r, "notes.txt").unwrap();
    assert!(!plain.contains('/'));
}

#[test]
fn trash_list_merges_disks() {
    let (d, disks) = two_disks();
    let r = d.path();
    trash_on(&disks, r, "notes.txt").unwrap();
    trash_on(&disks, r, "SD/films/a.mkv").unwrap();
    let mut origins: Vec<String> = list_trash(r).unwrap().into_iter().map(|t| t.origin).collect();
    origins.sort();
    assert_eq!(origins, ["SD/films/a.mkv", "notes.txt"]);
}

#[test]
fn restore_and_empty_across_disks() {
    let (d, disks) = two_disks();
    let r = d.path();
    let id = trash_on(&disks, r, "SD/films/a.mkv").unwrap();
    trash_on(&disks, r, "notes.txt").unwrap();
    assert_eq!(restore(r, &id).unwrap(), r.join("SD/films/a.mkv"));
    assert_eq!(fs::read_to_string(r.join("SD/films/a.mkv")).unwrap(), "film");
    trash_on(&disks, r, "SD/films/a.mkv").unwrap();
    assert_eq!(list_trash(r).unwrap().len(), 2);
    empty_trash(r).unwrap();
    assert!(list_trash(r).unwrap().is_empty());
    assert!(r.join("SD/.trash").is_dir(), "the card's trash folder stays, empty");
    assert_eq!(all_names(&r.join("SD/.trash")), Vec::<String>::new());
}

#[test]
fn move_across_disks_copies_then_removes() {
    let (d, disks) = two_disks();
    let r = d.path();
    let r2 = move_to_on(&disks, r, "Photos", "SD");
    assert!(matches!(r2, Err(Error::CrossDisk)), "{r2:?}");
    assert!(r.join("Photos/a.jpg").exists(), "nothing changed");
    assert!(move_to_on(&disks, r, "SD/films/a.mkv", "SD").is_ok(), "same disk is a plain move");
    let never = AtomicBool::new(false);
    let mut seen = vec![];
    let landed = move_across(r, "Photos", "SD", &never, &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(landed, r.join("SD/Photos"));
    assert_eq!(fs::read_to_string(r.join("SD/Photos/a.jpg")).unwrap(), "jpg");
    assert!(!r.join("Photos").exists());
    assert_eq!(seen.last(), Some(&(3, 3)));
    // A name taken on the other disk keeps both.
    fs::write(r.join("a.mkv"), "other").unwrap();
    assert_eq!(move_across(r, "a.mkv", "SD", &never, &mut |_, _| {}).unwrap(), r.join("SD/a 2.mkv"));
}

#[test]
fn cancelled_move_across_keeps_the_source() {
    let (d, _) = two_disks();
    let r = d.path();
    let cancel = AtomicBool::new(true);
    assert!(move_across(r, "Photos", "SD", &cancel, &mut |_, _| {}).is_err());
    assert!(r.join("Photos/a.jpg").exists());
    assert!(!r.join("SD/Photos").exists());
    assert!(all_names(&r.join("SD")).iter().all(|n| !n.starts_with(".binder-partial-")));
}

#[test]
fn list_hides_disk_trash_and_lost_found() {
    let (d, disks) = two_disks();
    let r = d.path();
    fs::create_dir(r.join("SD/lost+found")).unwrap();
    trash_on(&disks, r, "SD/films/a.mkv").unwrap();
    assert_eq!(names(&list(r, "SD", true).unwrap()), ["films"]);
    fs::create_dir(r.join("Photos/.trash")).unwrap();
    assert!(names(&list(r, "Photos", true).unwrap()).contains(&".trash"), "only a disk's own trash hides");
}
