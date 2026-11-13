//! Cloud: a small file manager over one folder, the mounted SMB share. Every path is relative to that root, and
//! nothing outside it is ever read or written. Deletes go to `.trash/` and can be restored.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The trash folder at the root. Each deleted item sits in its own `<id>/` folder beside an `.origin` file.
pub const TRASH: &str = ".trash";
const ORIGIN: &str = ".origin";
const PARTIAL: &str = ".binder-partial-";
const CHUNK: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Dir,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub name: String,
    /// Relative to the root, `/`-separated.
    pub rel: String,
    pub kind: Kind,
    pub size: u64,
    /// Unix seconds.
    pub modified: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrashEntry {
    pub id: String,
    /// Where it was, relative to the root.
    pub origin: String,
    /// Unix seconds.
    pub deleted: i64,
    pub entry: Entry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Space {
    pub total: u64,
    pub free: u64,
}

fn outside() -> Error {
    Error::Invalid("That path is outside Cloud.".into())
}

/// The real path for `rel`, which must stay inside `root`: no absolute paths, no `..`, and no symlink that leads out.
/// The path itself doesn't have to exist yet.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(outside());
    }
    let real_root = root.canonicalize()?;
    // Paths keep the root as the caller gave it; only the check uses the real one.
    let path = root.join(rel_path);
    // The deepest part that exists decides where the path really goes.
    let mut existing = path.as_path();
    while fs::symlink_metadata(existing).is_err() {
        existing = existing.parent().ok_or_else(outside)?;
    }
    let real = existing.canonicalize().map_err(|_| outside())?;
    if !real.starts_with(&real_root) {
        return Err(outside());
    }
    Ok(path)
}

fn checked_name(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() || n == "." || n == ".." || n.contains('/') || n.contains('\0') {
        return Err(Error::Invalid("A name can't be empty, . or .., or contain /.".into()));
    }
    Ok(n)
}

fn modified(meta: &fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

fn entry_of(path: &Path, rel: String) -> Result<Entry> {
    let meta = fs::symlink_metadata(path)?;
    let kind = if meta.file_type().is_symlink() {
        Kind::Link
    } else if meta.is_dir() {
        Kind::Dir
    } else {
        Kind::File
    };
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Entry { name, rel, kind, size: if kind == Kind::File { meta.len() } else { 0 }, modified: modified(&meta) })
}

fn join_rel(rel: &str, name: &str) -> String {
    let rel = rel.trim_matches('/');
    if rel.is_empty() { name.to_string() } else { format!("{rel}/{name}") }
}

fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| (a.kind != Kind::Dir).cmp(&(b.kind != Kind::Dir)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// A folder's entries: folders first, then by name. The trash and unfinished copies never show; dotfiles only when asked.
pub fn list(root: &Path, rel: &str, show_hidden: bool) -> Result<Vec<Entry>> {
    let dir = resolve(root, rel)?;
    let at_root = rel.trim_matches('/').is_empty();
    let mut out = vec![];
    for item in fs::read_dir(&dir)? {
        let name = item?.file_name().to_string_lossy().into_owned();
        if (at_root && name == TRASH) || name.starts_with(PARTIAL) || (!show_hidden && name.starts_with('.')) {
            continue;
        }
        out.push(entry_of(&dir.join(&name), join_rel(rel, &name))?);
    }
    sort(&mut out);
    Ok(out)
}

/// `name` in `dir`, or the first free `name 2.ext`, `name 3.ext`. Folders and names without an extension get `name 2`.
pub fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !first.is_dir() => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..).map(|n| dir.join(format!("{stem} {n}{ext}"))).find(|p| fs::symlink_metadata(p).is_err()).expect("some number is free")
}

fn size_of(path: &Path) -> Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        let mut total = 0;
        for item in fs::read_dir(path)? {
            total += size_of(&item?.path())?;
        }
        Ok(total)
    } else {
        Ok(meta.len())
    }
}

fn remove(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(_) => Ok(()),
    }
}

struct Copy<'a> {
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64, u64),
    done: u64,
    total: u64,
}

impl Copy<'_> {
    fn path(&mut self, from: &Path, to: &Path) -> Result<()> {
        let meta = fs::symlink_metadata(from)?;
        if meta.is_dir() {
            fs::create_dir(to)?;
            for item in fs::read_dir(from)? {
                let item = item?;
                self.path(&item.path(), &to.join(item.file_name()))?;
            }
            return Ok(());
        }
        if meta.file_type().is_symlink() {
            // A link is copied as the link, never followed.
            std::os::unix::fs::symlink(fs::read_link(from)?, to)?;
            return Ok(());
        }
        let mut src = fs::File::open(from)?;
        let mut dst = fs::File::create(to)?;
        let mut buf = vec![0u8; CHUNK];
        loop {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(Error::Invalid("Cancelled.".into()));
            }
            let n = src.read(&mut buf)?;
            if n == 0 {
                break;
            }
            dst.write_all(&buf[..n])?;
            self.done += n as u64;
            (self.progress)(self.done, self.total);
        }
        dst.flush()?;
        Ok(())
    }
}

/// Copies a file or folder into `dst_dir`, in 1 MB chunks, reporting `(done, total)` bytes. It writes to a hidden
/// partial name first and renames at the end, so a cancelled or failed copy leaves nothing behind.
/// Returns where it landed, or `None` when `Skip` met an existing name.
pub fn copy_into(src: &Path, dst_dir: &Path, conflict: Conflict, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<Option<PathBuf>> {
    let name = src.file_name().ok_or_else(|| Error::Invalid("Nothing to copy.".into()))?.to_string_lossy().into_owned();
    let mut target = dst_dir.join(&name);
    if fs::symlink_metadata(&target).is_ok() {
        match conflict {
            Conflict::Skip => return Ok(None),
            Conflict::KeepBoth => target = free_name(dst_dir, &name),
            Conflict::Replace => {}
        }
    }
    let partial = free_name(dst_dir, &format!("{PARTIAL}{name}"));
    let total = size_of(src)?;
    let mut copy = Copy { cancel, progress, done: 0, total };
    if let Err(e) = copy.path(src, &partial) {
        let _ = remove(&partial);
        return Err(e);
    }
    if total == 0 {
        (copy.progress)(0, 0);
    }
    if conflict == Conflict::Replace {
        remove(&target)?;
    }
    fs::rename(&partial, &target)?;
    Ok(Some(target))
}

pub fn mkdir(root: &Path, rel_dir: &str, name: &str) -> Result<PathBuf> {
    let name = checked_name(name)?;
    let path = resolve(root, &join_rel(rel_dir, name))?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(Error::Invalid(format!("There is already something called {name} here.")));
    }
    fs::create_dir(&path)?;
    Ok(path)
}

pub fn rename(root: &Path, rel: &str, new_name: &str) -> Result<PathBuf> {
    let name = checked_name(new_name)?;
    if rel.trim_matches('/').is_empty() {
        return Err(Error::Invalid("Cloud itself can't be renamed.".into()));
    }
    let from = resolve(root, rel)?;
    let to = from.with_file_name(name);
    if fs::symlink_metadata(&to).is_ok() {
        return Err(Error::Invalid(format!("There is already something called {name} here.")));
    }
    fs::rename(&from, &to)?;
    Ok(to)
}

/// Moves into another folder, keeping both on a name clash.
pub fn move_to(root: &Path, rel: &str, dest_dir_rel: &str) -> Result<PathBuf> {
    if rel.trim_matches('/').is_empty() {
        return Err(Error::Invalid("Cloud itself can't be moved.".into()));
    }
    let from = resolve(root, rel)?;
    let dest = resolve(root, dest_dir_rel)?;
    if !dest.is_dir() {
        return Err(Error::Invalid("Move into a folder.".into()));
    }
    if dest.canonicalize()?.starts_with(from.canonicalize()?) {
        return Err(Error::Invalid("A folder can't go inside itself.".into()));
    }
    let name = from.file_name().expect("not the root").to_string_lossy().into_owned();
    let to = free_name(&dest, &name);
    fs::rename(&from, &to)?;
    Ok(to)
}

fn trash_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(TRASH);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Moves an item to the trash, remembering where it was.
pub fn trash(root: &Path, rel: &str) -> Result<()> {
    let rel = rel.trim_matches('/');
    if rel.is_empty() || rel == TRASH || rel.starts_with(&format!("{TRASH}/")) {
        return Err(Error::Invalid("That can't go to the trash.".into()));
    }
    let from = resolve(root, rel)?;
    fs::symlink_metadata(&from)?;
    let bin = trash_dir(root)?;
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let slot = (0..)
        .map(|n| bin.join(format!("{ms}-{n}")))
        .find(|p| fs::create_dir(p).is_ok())
        .expect("a free slot");
    fs::write(slot.join(ORIGIN), rel)?;
    fs::rename(&from, slot.join(from.file_name().expect("not the root")))?;
    Ok(())
}

/// What's in the trash, newest first.
pub fn list_trash(root: &Path) -> Result<Vec<TrashEntry>> {
    let bin = trash_dir(root)?;
    let mut out = vec![];
    for slot in fs::read_dir(&bin)? {
        let slot = slot?.path();
        let id = slot.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let Ok(origin) = fs::read_to_string(slot.join(ORIGIN)) else { continue };
        let Some(item) = fs::read_dir(&slot)?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n != ORIGIN)) else {
            continue;
        };
        let deleted = id.split('-').next().and_then(|ms| ms.parse::<i64>().ok()).map_or(0, |ms| ms / 1000);
        let name = item.file_name().unwrap_or_default().to_string_lossy().into_owned();
        out.push(TrashEntry { entry: entry_of(&item, format!("{id}/{name}"))?, id, origin, deleted });
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

/// Puts a trashed item back where it was, recreating its folders, and keeps both on a clash.
pub fn restore(root: &Path, id: &str) -> Result<PathBuf> {
    let item = list_trash(root)?.into_iter().find(|t| t.id == id).ok_or_else(|| Error::Invalid("That's no longer in the trash.".into()))?;
    let origin = resolve(root, &item.origin)?;
    let parent = origin.parent().ok_or_else(outside)?;
    fs::create_dir_all(parent)?;
    let target = free_name(parent, &item.entry.name);
    let slot = trash_dir(root)?.join(id);
    fs::rename(slot.join(&item.entry.name), &target)?;
    fs::remove_dir_all(slot)?;
    Ok(target)
}

pub fn empty_trash(root: &Path) -> Result<()> {
    let bin = trash_dir(root)?;
    fs::remove_dir_all(&bin)?;
    fs::create_dir(&bin)?;
    Ok(())
}

/// The size and free space of the disk the root lives on.
pub fn space(root: &Path) -> Result<Space> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(root.as_os_str().as_bytes()).map_err(|_| outside())?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `s` a properly sized, writable statvfs.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let unit = s.f_frsize as u64;
    Ok(Space { total: s.f_blocks as u64 * unit, free: s.f_bavail as u64 * unit })
}
