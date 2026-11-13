//! The Cloud fast path, with a local `sh` playing the server: the mount and the server's folder are the same
//! temp folder here.

use binder_core::Error;
use binder_core::cloud::{self, Conflict, list_trash};
use binder_core::cloud_fast::Fast;
use binder_core::shell::LocalShell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

struct World {
    dir: tempfile::TempDir,
    remote: String,
}

impl World {
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("cloud/Photos/2026")).unwrap();
        fs::write(dir.path().join("cloud/Photos/a.jpg"), "jpg").unwrap();
        fs::write(dir.path().join("cloud/Photos/2026/b.jpg"), "bee").unwrap();
        std::os::unix::fs::symlink("a.jpg", dir.path().join("cloud/Photos/link")).unwrap();
        fs::create_dir_all(dir.path().join("mac")).unwrap();
        let remote = dir.path().join("cloud").to_string_lossy().into_owned();
        World { dir, remote }
    }
    fn root(&self) -> PathBuf {
        self.dir.path().join("cloud")
    }
    fn mac(&self) -> PathBuf {
        self.dir.path().join("mac")
    }
    fn fast(&self) -> Fast<'_> {
        static SHELL: LocalShell = LocalShell;
        Fast { shell: &SHELL, root: leak(self.root()), remote_root: leak_str(self.remote.clone()), gnu_tar: false }
    }
}

fn leak(p: PathBuf) -> &'static Path {
    Box::leak(p.into_boxed_path())
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Every item under `dir` with its bytes (files), its target (links) or nothing (folders).
fn tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    fn walk(base: &Path, at: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let m = fs::symlink_metadata(&p).unwrap();
            if m.file_type().is_symlink() {
                out.insert(rel, format!("-> {}", fs::read_link(&p).unwrap().display()));
            } else if m.is_dir() {
                out.insert(rel.clone(), "/".into());
                walk(base, &p, out);
            } else {
                out.insert(rel, String::from_utf8_lossy(&fs::read(&p).unwrap()).into_owned());
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

/// `dir` holds what `expected` holds. Paths are looked up rather than compared as text, because macOS's unzip
/// writes names decomposed (NFD) and APFS finds them either way; the Pi's ext4 keeps the bytes as sent.
fn same_tree(dir: &Path, expected: &Path, what: &str) {
    let want = tree(expected);
    assert_eq!(tree(dir).len(), want.len(), "{what}: item count");
    for (rel, value) in want {
        let p = dir.join(&rel);
        let m = fs::symlink_metadata(&p).unwrap_or_else(|_| panic!("{what}: {rel} missing"));
        let got = if m.file_type().is_symlink() {
            format!("-> {}", fs::read_link(&p).unwrap().display())
        } else if m.is_dir() {
            "/".into()
        } else {
            String::from_utf8_lossy(&fs::read(&p).unwrap()).into_owned()
        };
        assert_eq!(got, value, "{what}: {rel}");
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn partials(dir: &Path) -> Vec<String> {
    names(dir).into_iter().filter(|n| n.starts_with(".binder-partial-")).collect()
}

fn never() -> AtomicBool {
    AtomicBool::new(false)
}

fn many_files(dir: &Path, n: usize) {
    for i in 0..n {
        let d = dir.join(format!("node_modules/pkg{}", i / 40));
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join(format!("f{i}.js")), format!("module {i}")).unwrap();
    }
}

#[test]
fn check_true_when_both_see_the_file() {
    let w = World::new();
    assert!(w.fast().check());
    assert!(!w.root().join(".binder-check").exists());
}

#[test]
fn check_false_when_remote_root_is_wrong() {
    let w = World::new();
    let other = tempfile::tempdir().unwrap();
    let fast = Fast { remote_root: leak_str(other.path().to_string_lossy().into_owned()), ..w.fast() };
    assert!(!fast.check());
    assert!(!w.root().join(".binder-check").exists());
}

#[test]
fn upload_dir_copies_every_file() {
    let w = World::new();
    let src = w.mac().join("proj");
    many_files(&src, 1600);
    let mut seen = vec![];
    let landed = w.fast().upload_dir(&src, "", Conflict::KeepBoth, &never(), &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(landed, Some(w.root().join("proj")));
    assert_eq!(tree(&w.root().join("proj")), tree(&src));
    assert_eq!(tree(&src).values().filter(|v| v.starts_with("module")).count(), 1600);
    let (done, total) = *seen.last().unwrap();
    assert_eq!((done, total), (total, cloud::tree_size(&src, "").unwrap().0));
    assert!(partials(&w.root()).is_empty());
}

#[test]
fn upload_dir_conflict_rules() {
    let w = World::new();
    let src = w.mac().join("Photos");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("new.jpg"), "new").unwrap();
    let before = tree(&w.root().join("Photos"));
    let f = w.fast();
    assert_eq!(f.upload_dir(&src, "", Conflict::Skip, &never(), &mut |_, _| {}).unwrap(), None);
    assert_eq!(tree(&w.root().join("Photos")), before);
    assert_eq!(f.upload_dir(&src, "", Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap(), Some(w.root().join("Photos 2")));
    assert_eq!(f.upload_dir(&src, "", Conflict::Replace, &never(), &mut |_, _| {}).unwrap(), Some(w.root().join("Photos")));
    assert_eq!(tree(&w.root().join("Photos")), tree(&src));
    let trashed = list_trash(&w.root()).unwrap();
    assert_eq!(trashed.len(), 1);
    assert_eq!(trashed[0].origin, "Photos");
    assert!(partials(&w.root()).is_empty());
}

#[test]
fn download_dir_round_trip() {
    let w = World::new();
    let mut seen = vec![];
    let landed = w.fast().download_dir("Photos", &w.mac(), Conflict::KeepBoth, &never(), &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(landed, Some(w.mac().join("Photos")));
    assert_eq!(tree(&w.mac().join("Photos")), tree(&w.root().join("Photos")));
    assert_eq!(tree(&w.mac().join("Photos")).get("link").map(String::as_str), Some("-> a.jpg"));
    assert_eq!(seen.last().map(|s| s.0 == s.1), Some(true));
    assert_eq!(w.fast().download_dir("Photos", &w.mac(), Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap(), Some(w.mac().join("Photos 2")));
    assert!(w.fast().download_dir("Photos", &w.mac(), Conflict::Replace, &never(), &mut |_, _| {}).is_err(), "no trash on the Mac");
    assert!(partials(&w.mac()).is_empty());
}

#[test]
fn copy_into_itself_is_refused() {
    let w = World::new();
    let f = w.fast();
    let copy = f.copy("Photos", "", "Photos copy", Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap();
    assert_eq!(copy, Some(w.root().join("Photos copy")));
    assert_eq!(tree(&w.root().join("Photos copy")), tree(&w.root().join("Photos")));
    assert!(f.copy("Photos", "Photos", "x", Conflict::KeepBoth, &never(), &mut |_, _| {}).is_err());
    assert!(f.copy("Photos", "Photos/2026", "x", Conflict::KeepBoth, &never(), &mut |_, _| {}).is_err());
    assert!(!w.root().join("Photos/x").exists() && !w.root().join("Photos/2026/x").exists());
    let file = f.copy("Photos/a.jpg", "Photos/2026", "a.jpg", Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap();
    assert_eq!(fs::read_to_string(file.unwrap()).unwrap(), "jpg");
    assert!(partials(&w.root()).is_empty());
}

#[test]
fn extract_and_compress_round_trip() {
    let w = World::new();
    let f = w.fast();
    let mut seen = vec![];
    let zip = f.compress(&["Photos".into()], &never(), &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(zip, w.root().join("Photos.zip"));
    assert_eq!(seen.last(), Some(&(5, 5)), "Photos, a.jpg, 2026, 2026/b.jpg and the link");
    let mut seen = vec![];
    let out = f.extract("Photos.zip", &never(), &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(out, w.root().join("Photos 2"));
    assert_eq!(tree(&out.join("Photos")), tree(&w.root().join("Photos")));
    assert_eq!(seen.last(), Some(&(5, 5)));
    assert_eq!(f.compress(&["Photos/a.jpg".into(), "Photos/2026".into()], &never(), &mut |_, _| {}).unwrap(), w.root().join("Photos/Archive.zip"));
    assert_eq!(f.size("Photos").unwrap(), cloud::tree_size(&w.root(), "Photos").unwrap());
    assert_eq!(f.size("Photos/a.jpg").unwrap(), (3, 1));
    assert!(partials(&w.root()).is_empty());
}

#[test]
fn extract_refuses_dotdot() {
    let w = World::new();
    let mut z = zip::ZipWriter::new(fs::File::create(w.root().join("bad.zip")).unwrap());
    z.start_file("../evil", zip::write::SimpleFileOptions::default()).unwrap();
    std::io::Write::write_all(&mut z, b"evil").unwrap();
    z.finish().unwrap();
    let r = w.fast().extract("bad.zip", &never(), &mut |_, _| {});
    assert!(matches!(&r, Err(Error::Invalid(m)) if m == "This zip has paths that leave its folder."), "{r:?}");
    assert!(!w.dir.path().join("evil").exists());
    assert!(!w.root().join("bad").exists());
    assert!(partials(&w.root()).is_empty());
}

#[test]
fn cancel_removes_the_partial_on_both_sides() {
    let w = World::new();
    let src = w.mac().join("big");
    fs::create_dir_all(&src).unwrap();
    let noise: Vec<u8> = (0..8_000_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
    fs::write(src.join("blob.bin"), &noise).unwrap();
    let cancel = AtomicBool::new(false);
    let r = w.fast().upload_dir(&src, "", Conflict::KeepBoth, &cancel, &mut |_, _| cancel.store(true, Ordering::SeqCst));
    assert!(r.is_err());
    assert!(!w.root().join("big").exists());
    assert!(partials(&w.root()).is_empty(), "{:?}", names(&w.root()));
    fs::create_dir_all(w.root().join("big")).unwrap();
    fs::write(w.root().join("big/blob.bin"), &noise).unwrap();
    let cancel = AtomicBool::new(false);
    let r = w.fast().download_dir("big", &w.mac().join("big"), Conflict::KeepBoth, &cancel, &mut |_, _| cancel.store(true, Ordering::SeqCst));
    assert!(r.is_err());
    assert_eq!(names(&w.mac().join("big")), ["blob.bin"], "nothing left beside the source");
    let cancel = AtomicBool::new(true);
    assert!(w.fast().copy("big", "", "big copy", Conflict::KeepBoth, &cancel, &mut |_, _| {}).is_err());
    assert!(w.fast().compress(&["big".into()], &cancel, &mut |_, _| {}).is_err());
    assert_eq!(names(&w.root()), ["Photos", "big"]);
}

#[test]
fn awkward_names_survive_every_fast_op() {
    for name in ["it's $HOME", "`x`", "-rf", "çağrı.txt", "a b", "semi;colon && echo hi"] {
        let w = World::new();
        fs::write(w.root().join("sentinel"), "keep").unwrap();
        let src = w.mac().join(name);
        fs::create_dir_all(src.join(name)).unwrap();
        fs::write(src.join(name).join(name), name).unwrap();
        let f = w.fast();
        let up = f.upload_dir(&src, "", Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap().unwrap();
        assert_eq!(up, w.root().join(name), "{name}");
        same_tree(&up, &src, name);
        let copy = f.copy(name, "", &format!("{name} copy"), Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap().unwrap();
        same_tree(&copy, &src, name);
        let zip = f.compress(&[name.to_string()], &never(), &mut |_, _| {}).unwrap();
        let zip_name = zip.file_name().unwrap().to_string_lossy().into_owned();
        let out = f.extract(&zip_name, &never(), &mut |_, _| {}).unwrap();
        same_tree(&out.join(name), &src, name);
        assert_eq!(f.size(name).unwrap(), cloud::tree_size(&w.root(), name).unwrap(), "{name}");
        let down = f.download_dir(name, &w.mac().join("back"), Conflict::KeepBoth, &never(), &mut |_, _| {});
        assert!(down.is_err(), "the folder must exist first");
        fs::create_dir_all(w.mac().join("back")).unwrap();
        let down = f.download_dir(name, &w.mac().join("back"), Conflict::KeepBoth, &never(), &mut |_, _| {}).unwrap().unwrap();
        same_tree(&down, &src, name);
        let mut expected: Vec<String> = ["Photos", "sentinel", name].iter().map(|s| s.to_string()).collect();
        for p in [&copy, &zip, &out] {
            expected.push(p.file_name().unwrap().to_string_lossy().into_owned());
        }
        expected.sort();
        assert_eq!(names(&w.root()), expected, "{name}");
        assert_eq!(fs::read_to_string(w.root().join("sentinel")).unwrap(), "keep");
        assert_eq!(names(&w.mac()), ["back", name].iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>());
    }
}

#[test]
fn paths_outside_root_refused() {
    let w = World::new();
    let f = w.fast();
    std::os::unix::fs::symlink("/tmp", w.root().join("out")).unwrap();
    for rel in ["../x", "/etc", "out", "out/x"] {
        assert!(f.remote(rel).is_err(), "{rel}");
        assert!(f.size(rel).is_err(), "{rel}");
        assert!(f.copy(rel, "", "x", Conflict::KeepBoth, &never(), &mut |_, _| {}).is_err(), "{rel}");
        assert!(f.extract(rel, &never(), &mut |_, _| {}).is_err(), "{rel}");
        assert!(f.compress(&[rel.to_string()], &never(), &mut |_, _| {}).is_err(), "{rel}");
    }
    let src = w.mac().join("x");
    fs::create_dir_all(&src).unwrap();
    assert!(f.upload_dir(&src, "..", Conflict::KeepBoth, &never(), &mut |_, _| {}).is_err());
    assert!(f.upload_dir(&src, "out", Conflict::KeepBoth, &never(), &mut |_, _| {}).is_err());
    assert_eq!(f.remote("Photos/./a.jpg").unwrap(), format!("{}/Photos/a.jpg", w.remote));
    assert_eq!(f.remote("").unwrap(), w.remote);
}

#[test]
fn fast_move_across_and_space() {
    let w = World::new();
    fs::create_dir_all(w.root().join("SD")).unwrap();
    let f = w.fast();
    let mut seen = vec![];
    let landed = f.move_across("Photos", "SD", &never(), &mut |a, b| seen.push((a, b))).unwrap();
    assert_eq!(landed, w.root().join("SD/Photos"));
    assert_eq!(tree(&w.root().join("SD/Photos")).get("a.jpg").map(String::as_str), Some("jpg"));
    assert!(!w.root().join("Photos").exists());
    assert_eq!(seen.last().map(|s| s.0 == s.1), Some(true));
    let cancel = AtomicBool::new(true);
    assert!(f.move_across("SD/Photos", "", &cancel, &mut |_, _| {}).is_err());
    assert!(w.root().join("SD/Photos/a.jpg").exists(), "a cancel keeps the source");
    let s = f.space("SD").unwrap();
    assert!(s.total > 0 && s.free <= s.total, "{s:?}");
    let m = cloud::space(&w.root()).unwrap();
    assert!(s.total.abs_diff(m.total) < 1 << 20, "{s:?} {m:?}");
    assert!(f.space("../x").is_err());
}

#[test]
fn extract_a_zip_whose_name_has_wildcards() {
    let w = World::new();
    let f = w.fast();
    fs::create_dir_all(w.root().join("one")).unwrap();
    fs::write(w.root().join("one/wrong.txt"), "wrong").unwrap();
    f.compress(&["one".into()], &never(), &mut |_, _| {}).unwrap();
    fs::rename(w.root().join("one.zip"), w.root().join("Photos 1.zip")).unwrap();
    f.compress(&["Photos".into()], &never(), &mut |_, _| {}).unwrap();
    fs::rename(w.root().join("Photos.zip"), w.root().join("Photos [12].zip")).unwrap();
    fs::copy(w.root().join("Photos [12].zip"), w.root().join("a*b.zip")).unwrap();
    fs::copy(w.root().join("Photos 1.zip"), w.root().join("aXb.zip")).unwrap();
    let out = f.extract("Photos [12].zip", &never(), &mut |_, _| {}).unwrap();
    assert_eq!(out, w.root().join("Photos [12]"));
    assert!(out.join("Photos/a.jpg").exists(), "{:?}", names(&out));
    assert!(!out.join("one").exists(), "the other zip was unpacked");
    let star = f.extract("a*b.zip", &never(), &mut |_, _| {}).unwrap();
    assert!(star.join("Photos/a.jpg").exists() && !star.join("one").exists());
    assert!(partials(&w.root()).is_empty(), "{:?}", names(&w.root()));
}

#[test]
fn upload_or_download_a_folder_into_itself_is_refused() {
    let w = World::new();
    let f = w.fast();
    let inside = |r: binder_core::Result<Option<PathBuf>>| matches!(&r, Err(Error::Invalid(m)) if m == "A folder can't go inside itself.");
    let up = f.upload_dir(&w.root().join("Photos"), "Photos/2026", Conflict::KeepBoth, &never(), &mut |_, _| {});
    assert!(inside(up));
    let down = f.download_dir("Photos", &w.root().join("Photos/2026"), Conflict::KeepBoth, &never(), &mut |_, _| {});
    assert!(inside(down));
    assert_eq!(names(&w.root().join("Photos/2026")), vec!["b.jpg"]);
    assert!(partials(&w.root()).is_empty());
}
