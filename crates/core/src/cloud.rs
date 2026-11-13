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
/// In the root's trash: the other disks' roots that hold a trash of their own, one relative path per line.
const DISKS: &str = ".disks";
/// ext4 makes it at a disk's root; it is root-owned and never the owner's.
const LOST_FOUND: &str = "lost+found";

/// How items are renamed. The share reports a rename from one disk to another as EXDEV; through SMB that is the
/// only sign of a disk boundary the Mac gets. Tests fake it.
pub trait Disks {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
}

pub struct RealDisks;

impl Disks for RealDisks {
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }
}

fn cross_disk(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::EXDEV)
}
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
    /// A copy that never finished, swept from a folder. Its entry carries the name it would have had.
    pub unfinished: bool,
    pub entry: Entry,
}

/// What Get info shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Info {
    pub entry: Entry,
    /// Unix seconds; 0 when the share doesn't say.
    pub created: i64,
    /// The item's path on this Mac.
    pub path: String,
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
    let here = rel.trim_matches('/');
    let disk_root = here.is_empty() || disk_roots(root).iter().any(|d| d == here);
    let mut out = vec![];
    for item in fs::read_dir(&dir)? {
        let name = item?.file_name().to_string_lossy().into_owned();
        if (disk_root && name == TRASH) || name == LOST_FOUND || name.starts_with(PARTIAL) || (!show_hidden && name.starts_with('.')) {
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

/// Finder's name for a duplicate: `name copy.ext`, then `name copy 2.ext`. Folders and names without an
/// extension get `name copy`, `name copy 2`.
pub fn copy_name(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !dir.join(name).is_dir() => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    std::iter::once(dir.join(format!("{stem} copy{ext}")))
        .chain((2..).map(|n| dir.join(format!("{stem} copy {n}{ext}"))))
        .find(|p| fs::symlink_metadata(p).is_err())
        .expect("some number is free")
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

pub(crate) fn remove(path: &Path) -> std::io::Result<()> {
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
        // Never truncate something that is already there.
        let mut dst = fs::File::create_new(to)?;
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
///
/// The conflict choice is applied when the copy lands, not when it starts: a name taken meanwhile (another copy,
/// the phone) is never overwritten. `Replace` moves the old item to the trash of `trash_root`, so nothing is
/// deleted outright; without a Cloud root to trash into, `Replace` is refused.
/// Returns where it landed, or `None` when `Skip` met an existing name.
pub fn copy_into(
    src: &Path,
    dst_dir: &Path,
    conflict: Conflict,
    trash_root: Option<&Path>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Option<PathBuf>> {
    let name = src.file_name().ok_or_else(|| Error::Invalid("Nothing to copy.".into()))?.to_string_lossy().into_owned();
    copy_into_as(src, dst_dir, &name, conflict, trash_root, cancel, progress)
}

/// `copy_into` under another name, as Duplicate does.
pub fn copy_into_as(
    src: &Path,
    dst_dir: &Path,
    name: &str,
    conflict: Conflict,
    trash_root: Option<&Path>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Option<PathBuf>> {
    let name = checked_name(name)?.to_string();
    if conflict == Conflict::Replace && trash_root.is_none() {
        return Err(Error::Invalid("Replace only works inside Cloud.".into()));
    }
    let taken = |p: &Path| fs::symlink_metadata(p).is_ok();
    if conflict == Conflict::Skip && taken(&dst_dir.join(&name)) {
        return Ok(None);
    }
    let partial = dst_dir.join(partial_name(&name));
    let total = size_of(src)?;
    let mut copy = Copy { cancel, progress, done: 0, total };
    if let Err(e) = copy.path(src, &partial) {
        let _ = remove(&partial);
        return Err(e);
    }
    if total == 0 {
        (copy.progress)(0, 0);
    }
    let landed = land(&partial, dst_dir, &name, conflict, trash_root);
    if !matches!(landed, Ok(Some(_))) {
        let _ = remove(&partial);
    }
    landed
}

/// A partial name no other copy, in this process or another, can pick: `.binder-partial-<pid>-<n>-<name>`.
pub fn partial_name(name: &str) -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    format!("{PARTIAL}{}-{n}-{name}", std::process::id())
}

/// Moves a finished partial to its name, deciding the conflict now. A replaced item goes to the trash first and
/// comes back if the rename fails. A partial written over SSH can take a moment to show through the SMB mount,
/// so a missing one is looked for again every 100 ms for up to 2 s.
pub fn land(partial: &Path, dst_dir: &Path, name: &str, conflict: Conflict, trash_root: Option<&Path>) -> Result<Option<PathBuf>> {
    for _ in 0..20 {
        if fs::symlink_metadata(partial).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    fs::symlink_metadata(partial)?;
    let target = dst_dir.join(name);
    if fs::symlink_metadata(&target).is_err() {
        fs::rename(partial, &target)?;
        return Ok(Some(target));
    }
    match conflict {
        Conflict::Skip => Ok(None),
        Conflict::KeepBoth => {
            let target = free_name(dst_dir, name);
            fs::rename(partial, &target)?;
            Ok(Some(target))
        }
        Conflict::Replace => {
            let root = trash_root.expect("checked before copying");
            let rel = target.strip_prefix(root).map_err(|_| outside())?.to_string_lossy().into_owned();
            let id = trash(root, &rel)?;
            if let Err(e) = fs::rename(partial, &target) {
                let _ = restore(root, &id);
                return Err(e.into());
            }
            Ok(Some(target))
        }
    }
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
    move_to_on(&RealDisks, root, rel, dest_dir_rel)
}

/// `move_to`, renaming through `disks`. A move the share refuses as crossing disks gives `Error::CrossDisk` with
/// nothing changed; `move_across` does those.
pub fn move_to_on(disks: &dyn Disks, root: &Path, rel: &str, dest_dir_rel: &str) -> Result<PathBuf> {
    let (from, dest) = move_ends(root, rel, dest_dir_rel)?;
    let name = from.file_name().expect("not the root").to_string_lossy().into_owned();
    let to = free_name(&dest, &name);
    match disks.rename(&from, &to) {
        Ok(()) => Ok(to),
        Err(e) if cross_disk(&e) => Err(Error::CrossDisk),
        Err(e) => Err(e.into()),
    }
}

/// A move to another disk: a copy into a partial on the target disk, then the source is removed, only once the copy
/// has landed. A cancelled or failed copy leaves the source as it was. Progress counts bytes.
pub fn move_across(root: &Path, rel: &str, dest_dir_rel: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
    let (from, dest) = move_ends(root, rel, dest_dir_rel)?;
    let landed = copy_into(&from, &dest, Conflict::KeepBoth, Some(root), cancel, progress)?.expect("KeepBoth always lands");
    remove(&from)?;
    Ok(landed)
}

/// The two ends of a move, checked: not Cloud itself, into a folder, and not into itself.
fn move_ends(root: &Path, rel: &str, dest_dir_rel: &str) -> Result<(PathBuf, PathBuf)> {
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
    Ok((from, dest))
}

fn trash_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(TRASH);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Moves an item to the trash, remembering where it was. Returns its trash id.
pub fn trash(root: &Path, rel: &str) -> Result<String> {
    trash_on(&RealDisks, root, rel)
}

/// The disks' roots that hold a trash, relative to Cloud's root: the root itself (`""`) first, then those found.
fn disk_roots(root: &Path) -> Vec<String> {
    let listed = fs::read_to_string(root.join(TRASH).join(DISKS)).unwrap_or_default();
    let mut out = vec![String::new()];
    for line in listed.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !out.iter().any(|d| d == line) && resolve(root, line).is_ok_and(|p| p.is_dir()) {
            out.push(line.to_string());
        }
    }
    out
}

fn record_disk(root: &Path, disk: &str) -> Result<()> {
    if disk_roots(root).iter().any(|d| d == disk) {
        return Ok(());
    }
    let path = trash_dir(root)?.join(DISKS);
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(format!("{disk}\n").as_bytes())?;
    Ok(())
}

fn disk_dir(root: &Path, disk: &str) -> PathBuf {
    if disk.is_empty() { root.to_path_buf() } else { root.join(disk) }
}

/// A trash id names its slot, after the disk's root when that isn't Cloud's: `<slot>` or `SD/<slot>`.
fn slot_path(root: &Path, id: &str) -> PathBuf {
    let (disk, slot) = id.rsplit_once('/').unwrap_or(("", id));
    disk_dir(root, disk).join(TRASH).join(slot)
}

/// `trash`, renaming through `disks`. The item goes to the `.trash` at the root of its own disk: Cloud's first, then
/// each folder on the way down to the item, until the share takes the rename. A try that crosses disks leaves
/// nothing behind.
pub fn trash_on(disks: &dyn Disks, root: &Path, rel: &str) -> Result<String> {
    let rel = rel.trim_matches('/');
    let in_a_trash = disk_roots(root).iter().any(|d| {
        let bin = if d.is_empty() { TRASH.to_string() } else { format!("{d}/{TRASH}") };
        rel == bin || rel.starts_with(&format!("{bin}/"))
    });
    if rel.is_empty() || in_a_trash {
        return Err(Error::Invalid("That can't go to the trash.".into()));
    }
    let from = resolve(root, rel)?;
    fs::symlink_metadata(&from)?;
    let name = from.file_name().expect("not the root").to_owned();
    let parts: Vec<&str> = rel.split('/').collect();
    for depth in 0..parts.len() {
        let disk = parts[..depth].join("/");
        let bin = disk_dir(root, &disk).join(TRASH);
        let made_bin = !bin.exists();
        fs::create_dir_all(&bin)?;
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let slot = (0..).map(|n| bin.join(format!("{ms}-{n}"))).find(|p| fs::create_dir(p).is_ok()).expect("a free slot");
        let undo = |slot: &Path| {
            let _ = fs::remove_dir_all(slot);
            if made_bin {
                let _ = fs::remove_dir(&bin);
            }
        };
        if let Err(e) = fs::write(slot.join(ORIGIN), rel) {
            undo(&slot);
            return Err(e.into());
        }
        match disks.rename(&from, &slot.join(&name)) {
            Ok(()) => {
                let slot_name = slot.file_name().expect("a slot name").to_string_lossy().into_owned();
                if disk.is_empty() {
                    return Ok(slot_name);
                }
                record_disk(root, &disk)?;
                return Ok(format!("{disk}/{slot_name}"));
            }
            Err(e) if cross_disk(&e) => undo(&slot),
            Err(e) => {
                undo(&slot);
                return Err(e.into());
            }
        }
    }
    Err(Error::Invalid("No trash on that item's disk took it.".into()))
}

/// What's in the trash on every disk, newest first.
pub fn list_trash(root: &Path) -> Result<Vec<TrashEntry>> {
    trash_dir(root)?;
    let mut out = vec![];
    for disk in disk_roots(root) {
        let bin = disk_dir(root, &disk).join(TRASH);
        let Ok(slots) = fs::read_dir(&bin) else { continue };
        for slot in slots {
            let slot = slot?.path();
            let slot_name = slot.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let id = if disk.is_empty() { slot_name.clone() } else { format!("{disk}/{slot_name}") };
            let Ok(origin) = fs::read_to_string(slot.join(ORIGIN)) else { continue };
            let Some(item) = fs::read_dir(&slot)?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n != ORIGIN)) else {
                continue;
            };
            let deleted = slot_name.split('-').next().and_then(|ms| ms.parse::<i64>().ok()).map_or(0, |ms| ms / 1000);
            let name = item.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let mut entry = entry_of(&item, format!("{id}/{name}"))?;
            let unfinished = unfinished_name(&name).is_some();
            if let Some(real) = unfinished_name(&name) {
                entry.name = real.to_string();
            }
            out.push((slot_name, TrashEntry { entry, id, origin, deleted, unfinished }));
        }
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out.into_iter().map(|(_, t)| t).collect())
}

/// Puts a trashed item back where it was, recreating its folders, and keeps both on a clash.
pub fn restore(root: &Path, id: &str) -> Result<PathBuf> {
    let item = list_trash(root)?.into_iter().find(|t| t.id == id).ok_or_else(|| Error::Invalid("That's no longer in the trash.".into()))?;
    if item.unfinished {
        return Err(Error::Invalid("An unfinished upload can't be restored.".into()));
    }
    let origin = resolve(root, &item.origin)?;
    let parent = origin.parent().ok_or_else(outside)?;
    fs::create_dir_all(parent)?;
    let slot = slot_path(root, id);
    let stored = fs::read_dir(&slot)?.filter_map(|e| e.ok()).map(|e| e.file_name()).find(|n| n != ORIGIN).ok_or_else(|| Error::Invalid("That's no longer in the trash.".into()))?;
    let target = free_name(parent, &stored.to_string_lossy());
    fs::rename(slot.join(&stored), &target)?;
    fs::remove_dir_all(slot)?;
    Ok(target)
}

/// Empties every disk's trash. The list of disks stays.
pub fn empty_trash(root: &Path) -> Result<()> {
    let disks = disk_roots(root);
    let listed = fs::read_to_string(root.join(TRASH).join(DISKS)).ok();
    for disk in &disks {
        let bin = disk_dir(root, disk).join(TRASH);
        if bin.exists() {
            fs::remove_dir_all(&bin)?;
        }
        fs::create_dir(&bin)?;
    }
    if let Some(listed) = listed {
        fs::write(root.join(TRASH).join(DISKS), listed)?;
    }
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

/// The name a partial would have landed as, and the process that writes it.
fn partial_parts(name: &str) -> Option<(u32, &str)> {
    let rest = name.strip_prefix(PARTIAL)?;
    let (pid, rest) = rest.split_once('-')?;
    let (_, real) = rest.split_once('-')?;
    Some((pid.parse().ok()?, real))
}

fn unfinished_name(name: &str) -> Option<&str> {
    partial_parts(name).map(|(_, real)| real)
}

/// Moves the partials in one folder whose process has ended to the trash, where they show as unfinished uploads.
/// Partials of copies still running are left alone. Returns how many were moved.
pub fn sweep_partials(root: &Path, rel_dir: &str, alive: &dyn Fn(u32) -> bool) -> Result<usize> {
    let dir = resolve(root, rel_dir)?;
    let mut moved = 0;
    for item in fs::read_dir(&dir)? {
        let name = item?.file_name().to_string_lossy().into_owned();
        let Some((pid, _)) = partial_parts(&name) else { continue };
        if !alive(pid) {
            trash(root, &join_rel(rel_dir, &name))?;
            moved += 1;
        }
    }
    Ok(moved)
}

pub fn info(root: &Path, rel: &str) -> Result<Info> {
    let path = resolve(root, rel)?;
    let entry = entry_of(&path, rel.trim_matches('/').to_string())?;
    let meta = fs::symlink_metadata(&path)?;
    let created = meta.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
    Ok(Info { entry, created, path: path.to_string_lossy().into_owned() })
}

fn walk(path: &Path, bytes: &mut u64, items: &mut u64) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        for item in fs::read_dir(path)? {
            *items += 1;
            walk(&item?.path(), bytes, items)?;
        }
    } else {
        *bytes += meta.len();
    }
    Ok(())
}

/// A file's size, or a folder's total bytes and how many items it holds, links not followed. A file counts as one.
pub fn tree_size(root: &Path, rel: &str) -> Result<(u64, u64)> {
    let path = resolve(root, rel)?;
    let (mut bytes, mut items) = (0, 0);
    walk(&path, &mut bytes, &mut items)?;
    if !fs::symlink_metadata(&path)?.is_dir() {
        items = 1;
    }
    Ok((bytes, items))
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) { Err(Error::Invalid("Cancelled.".into())) } else { Ok(()) }
}

fn count(path: &Path) -> Result<u64> {
    let (mut bytes, mut items) = (0, 1);
    walk(path, &mut bytes, &mut items)?;
    Ok(items)
}

struct Zipper<'a, W: Write + std::io::Seek> {
    zip: zip::ZipWriter<W>,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64, u64),
    done: u64,
    total: u64,
}

impl<W: Write + std::io::Seek> Zipper<'_, W> {
    fn add(&mut self, path: &Path, name: &str) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        cancelled(self.cancel)?;
        let meta = fs::symlink_metadata(path)?;
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(meta.permissions().mode() & 0o777)
            .large_file(meta.len() >= u32::MAX as u64);
        let bad = |e: zip::result::ZipError| Error::Invalid(format!("Couldn't write the zip: {e}"));
        if meta.file_type().is_symlink() {
            self.zip.add_symlink(name, fs::read_link(path)?.to_string_lossy(), options).map_err(bad)?;
        } else if meta.is_dir() {
            self.zip.add_directory(format!("{name}/"), options).map_err(bad)?;
        } else {
            self.zip.start_file(name, options).map_err(bad)?;
            let mut src = fs::File::open(path)?;
            let mut buf = vec![0u8; CHUNK];
            loop {
                cancelled(self.cancel)?;
                let n = src.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                self.zip.write_all(&buf[..n])?;
            }
        }
        self.done += 1;
        (self.progress)(self.done, self.total);
        if meta.is_dir() && !meta.file_type().is_symlink() {
            let mut children: Vec<_> = fs::read_dir(path)?.collect::<std::io::Result<_>>()?;
            children.sort_by_key(|c| c.file_name());
            for c in children {
                self.add(&c.path(), &format!("{name}/{}", c.file_name().to_string_lossy()))?;
            }
        }
        Ok(())
    }
}

/// Where a zip of `rels` goes: their shared folder, each item's path, and the zip's name before a free one is picked.
pub(crate) struct ZipPlan {
    pub rel_dir: String,
    pub dir: PathBuf,
    pub items: Vec<PathBuf>,
    pub name: String,
}

pub(crate) fn zip_plan(root: &Path, rels: &[String]) -> Result<ZipPlan> {
    let parent_of = |rel: &str| rel.trim_matches('/').rsplit_once('/').map_or(String::new(), |(dir, _)| dir.to_string());
    let first = rels.first().ok_or_else(|| Error::Invalid("Nothing to compress.".into()))?;
    let rel_dir = parent_of(first);
    if rels.iter().any(|r| parent_of(r) != rel_dir || r.trim_matches('/').is_empty()) {
        return Err(Error::Invalid("Compress items from one folder.".into()));
    }
    let dir = resolve(root, &rel_dir)?;
    let items: Vec<PathBuf> = rels.iter().map(|r| resolve(root, r)).collect::<Result<_>>()?;
    for item in &items {
        fs::symlink_metadata(item)?;
    }
    let name = match items.as_slice() {
        [one] => format!("{}.zip", one.file_name().expect("not the root").to_string_lossy()),
        _ => "Archive.zip".into(),
    };
    Ok(ZipPlan { rel_dir, dir, items, name })
}

/// Zips items from one folder into that folder: one item makes `<its name>.zip`, several make `Archive.zip`, with a
/// free name when taken. Written to a partial and landed at the end; links are stored as links. Progress counts
/// items. This is the fallback when the server can't run `zip` itself.
pub fn compress_local(root: &Path, rels: &[String], cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
    let ZipPlan { dir, items, name, .. } = zip_plan(root, rels)?;
    let total = items.iter().map(|p| count(p)).sum::<Result<u64>>()?;
    let partial = dir.join(partial_name(&name));
    let made = (|| -> Result<()> {
        let file = fs::File::create_new(&partial)?;
        let mut z = Zipper { zip: zip::ZipWriter::new(std::io::BufWriter::new(file)), cancel, progress, done: 0, total };
        for item in &items {
            z.add(item, &item.file_name().expect("not the root").to_string_lossy())?;
        }
        let mut out = z.zip.finish().map_err(|e| Error::Invalid(format!("Couldn't write the zip: {e}")))?;
        out.flush()?;
        Ok(())
    })();
    let landed = made.and_then(|()| land(&partial, &dir, &name, Conflict::KeepBoth, None));
    match landed {
        Ok(Some(path)) => Ok(path),
        Ok(None) => unreachable!("KeepBoth always lands"),
        Err(e) => {
            let _ = remove(&partial);
            Err(e)
        }
    }
}

/// True for a zip entry name that is absolute or climbs out with `..`.
pub(crate) fn leaves(name: &str) -> bool {
    name.starts_with('/') || name.starts_with('\\') || name.split(['/', '\\']).any(|part| part == "..")
}

pub(crate) fn escapes() -> Error {
    Error::Invalid("This zip has paths that leave its folder.".into())
}

/// True when some folder between `base` and `path` is a link, so writing there would land somewhere else.
fn through_link(base: &Path, path: &Path) -> bool {
    let mut at = path.parent();
    while let Some(dir) = at {
        if dir == base {
            return false;
        }
        if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink()) {
            return true;
        }
        at = dir.parent();
    }
    false
}

fn unpack(zip: &mut zip::ZipArchive<fs::File>, into: &Path, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    const S_IFMT: u32 = 0o170000;
    const S_IFLNK: u32 = 0o120000;
    let bad = |e: zip::result::ZipError| Error::Invalid(format!("Couldn't read the zip: {e}"));
    let total = zip.len() as u64;
    for i in 0..zip.len() {
        cancelled(cancel)?;
        let mut file = zip.by_index(i).map_err(bad)?;
        let name = file.name().to_string();
        let path = into.join(name.trim_end_matches('/'));
        if through_link(into, &path) {
            return Err(escapes());
        }
        let mode = file.unix_mode();
        if file.is_dir() {
            fs::create_dir_all(&path)?;
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            if mode.is_some_and(|m| m & S_IFMT == S_IFLNK) {
                let mut target = String::new();
                file.read_to_string(&mut target)?;
                std::os::unix::fs::symlink(target, &path)?;
            } else {
                let mut dst = fs::File::create_new(&path)?;
                let mut buf = vec![0u8; CHUNK];
                loop {
                    cancelled(cancel)?;
                    let n = file.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    dst.write_all(&buf[..n])?;
                }
                if let Some(m) = mode {
                    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(m & 0o777));
                }
            }
        }
        progress(i as u64 + 1, total);
    }
    Ok(())
}

/// Unzips into a folder beside the zip named after it (a free name when taken), through a partial folder landed at
/// the end. Refuses a zip with any entry that is absolute or holds `..`. Progress counts entries. This is the
/// fallback when the server can't run `unzip` itself.
pub fn extract_local(root: &Path, rel: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
    let path = resolve(root, rel)?;
    let dir = path.parent().ok_or_else(outside)?.to_path_buf();
    let stem = path.file_stem().ok_or_else(|| Error::Invalid("Nothing to extract.".into()))?.to_string_lossy().into_owned();
    let mut zip = zip::ZipArchive::new(fs::File::open(&path)?).map_err(|e| Error::Invalid(format!("Couldn't read the zip: {e}")))?;
    if zip.file_names().any(leaves) {
        return Err(escapes());
    }
    let partial = dir.join(partial_name(&stem));
    let made = fs::create_dir(&partial).map_err(Error::from).and_then(|()| unpack(&mut zip, &partial, cancel, progress));
    match made.and_then(|()| land(&partial, &dir, &stem, Conflict::KeepBoth, None)) {
        Ok(Some(out)) => Ok(out),
        Ok(None) => unreachable!("KeepBoth always lands"),
        Err(e) => {
            let _ = remove(&partial);
            Err(e)
        }
    }
}
