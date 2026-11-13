//! The Cloud fast path: work that would crawl over SMB file by file runs on the server instead, over SSH. Folder
//! uploads and downloads stream one tar, copies run `cp -a`, zips run `zip` and `unzip`, and sizes are counted
//! there. Each one writes to a partial name and lands through the mount with `cloud::land`, so the conflict
//! rules, the trash and the checks on paths are the same as on SMB.

use crate::cloud::{self, Conflict, escapes, leaves, partial_name, resolve};
use crate::remote::quote;
use crate::shell::{Shell, run};
use crate::{Error, Result};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

const CHUNK: usize = 1024 * 1024;
const TAR: &str = "/usr/bin/tar";

pub struct Fast<'a> {
    pub shell: &'a dyn Shell,
    /// Where the share is mounted on this Mac.
    pub root: &'a Path,
    /// The same folder's path on the server.
    pub remote_root: &'a str,
    /// The server's tar is GNU tar, which needs telling to skip the keywords macOS's tar adds.
    pub gnu_tar: bool,
}

fn cancelled() -> Error {
    Error::Invalid("Cancelled.".into())
}

fn drain<R: Read + Send + 'static>(r: Option<R>) -> JoinHandle<String> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut r) = r {
            let _ = r.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn finish(child: &mut Child, err: JoinHandle<String>) -> Result<()> {
    let status = child.wait()?;
    let stderr = err.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let stderr = stderr.trim();
    Err(Error::Remote(if stderr.is_empty() { format!("The command ended with {status}.") } else { stderr.to_string() }))
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Waits up to 2 s for a child that was asked to stop, then kills it.
fn settle(child: &mut Child) {
    for _ in 0..20 {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    stop(child);
}

/// Copies one stream into another in chunks, checking `cancel` between them.
fn pump(from: &mut dyn Read, to: &mut dyn Write, cancel: &AtomicBool, sent: &mut dyn FnMut(u64)) -> Result<()> {
    let mut buf = vec![0u8; CHUNK];
    let mut total = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(cancelled());
        }
        let n = from.read(&mut buf)?;
        if n == 0 {
            break;
        }
        to.write_all(&buf[..n])?;
        total += n as u64;
        sent(total);
    }
    to.flush()?;
    Ok(())
}

/// Runs `command` in the background and stops it when the script's input closes, which is how a cancel or a lost
/// SSH connection reaches the server.
fn guarded(command: &str) -> String {
    // A background job's input is /dev/null, so the watcher gets the script's own input through fd 3.
    format!("exec 3<&0; {command} </dev/null & pid=$!; (cat <&3 >/dev/null; kill $pid 2>/dev/null) >/dev/null 2>&1 & exec 3<&-; wait $pid")
}

fn last_name(rel: &str) -> Result<String> {
    let rel = rel.trim_matches('/');
    match rel.rsplit('/').next() {
        Some(name) if !name.is_empty() && name != "." && name != ".." => Ok(name.to_string()),
        _ => Err(Error::Invalid("That needs an item inside Cloud.".into())),
    }
}

fn parent_rel(rel: &str) -> String {
    rel.trim_matches('/').rsplit_once('/').map_or(String::new(), |(dir, _)| dir.to_string())
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn plain_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n == "." || n == ".." || n.contains('/') || n.contains('\0') {
        return Err(Error::Invalid("A name can't be empty, . or .., or contain /.".into()));
    }
    Ok(n.to_string())
}

impl Fast<'_> {
    fn base(&self) -> &str {
        let b = self.remote_root.trim_end_matches('/');
        if b.is_empty() { "/" } else { b }
    }

    fn join(&self, rel_remote: &str, name: &str) -> String {
        format!("{}/{name}", rel_remote.trim_end_matches('/'))
    }

    /// Whether the server's folder is the mounted one: a token written over the mount is read back over SSH.
    pub fn check(&self) -> bool {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let token = format!("binder-{}-{nanos}", std::process::id());
        let local = self.root.join(".binder-check");
        if fs::write(&local, &token).is_err() {
            return false;
        }
        let seen = run(self.shell, &format!("cat {}", quote(&self.join(self.base(), ".binder-check"))));
        let _ = fs::remove_file(&local);
        seen.is_ok_and(|s| s.trim() == token)
    }

    /// The server's path for `rel`, after the same checks as on the mount.
    pub fn remote(&self, rel: &str) -> Result<String> {
        resolve(self.root, rel)?;
        let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        Ok(if parts.is_empty() { self.base().to_string() } else { self.join(self.base(), &parts.join("/")) })
    }

    fn cleanup(&self, remote_partial: &str, local_partial: &Path) {
        let _ = run(self.shell, &format!("rm -rf {}", quote(remote_partial)));
        let _ = cloud::remove(local_partial);
    }

    /// Runs a script, handing each line it prints to `line`, until it ends or `cancel` is set. Its input stays open
    /// meanwhile, so commands run through `guarded` stop when it closes.
    fn watch(&self, script: &str, cancel: &AtomicBool, line: &mut dyn FnMut(&str)) -> Result<()> {
        let mut child = self.shell.spawn(script)?;
        let input = child.stdin.take();
        let err = drain(child.stderr.take());
        let out = child.stdout.take().expect("stdout is piped");
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            for l in BufReader::new(out).split(b'\n') {
                let Ok(l) = l else { break };
                if tx.send(String::from_utf8_lossy(&l).into_owned()).is_err() {
                    break;
                }
            }
        });
        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(input);
                settle(&mut child);
                return Err(cancelled());
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(l) => line(&l),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let done = finish(&mut child, err);
        drop(input);
        done
    }

    /// Lands a finished partial, removing it when it didn't land.
    fn land(&self, local_partial: &Path, remote_partial: &str, dir: &Path, name: &str, conflict: Conflict, trash: bool) -> Result<Option<PathBuf>> {
        let landed = cloud::land(local_partial, dir, name, conflict, trash.then_some(self.root));
        if !matches!(landed, Ok(Some(_))) {
            self.cleanup(remote_partial, local_partial);
        }
        landed
    }

    /// Sends a Mac folder into `dst_rel` as one tar stream. Progress counts bytes against the folder's size.
    pub fn upload_dir(&self, src: &Path, dst_rel: &str, conflict: Conflict, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<Option<PathBuf>> {
        let name = src.file_name().ok_or_else(|| Error::Invalid("Nothing to upload.".into()))?.to_string_lossy().into_owned();
        let dst_dir = resolve(self.root, dst_rel)?;
        if !dst_dir.is_dir() {
            return Err(Error::Invalid("Upload into a folder.".into()));
        }
        cloud::not_inside(src, &dst_dir)?;
        if conflict == Conflict::Skip && exists(&dst_dir.join(&name)) {
            return Ok(None);
        }
        let (total, _) = cloud::tree_size(src, "")?;
        let partial = partial_name(&name);
        let remote_partial = self.join(&self.remote(dst_rel)?, &partial);
        let local_partial = dst_dir.join(&partial);
        let warn = if self.gnu_tar { "--warning=no-unknown-keyword " } else { "" };
        let mut server = self.shell.spawn(&format!("mkdir {p} && exec tar {warn}-xf - -C {p}", p = quote(&remote_partial)))?;
        let server_err = drain(server.stderr.take());
        let _server_out = drain(server.stdout.take());
        let tar = Command::new(TAR)
            .env("COPYFILE_DISABLE", "1")
            .args(["--no-mac-metadata", "--no-xattrs", "-cf", "-", "-C"])
            .arg(src)
            .arg(".")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut tar = match tar {
            Ok(t) => t,
            Err(e) => {
                stop(&mut server);
                self.cleanup(&remote_partial, &local_partial);
                return Err(e.into());
            }
        };
        let tar_err = drain(tar.stderr.take());
        let mut from = tar.stdout.take().expect("stdout is piped");
        let mut to = server.stdin.take().expect("stdin is piped");
        let pumped = pump(&mut from, &mut to, cancel, &mut |sent| progress(sent.min(total), total));
        drop(to);
        drop(from);
        let result = match pumped {
            Err(e @ Error::Invalid(_)) => {
                stop(&mut tar);
                settle(&mut server);
                Err(e)
            }
            // A broken pipe means the server stopped first; its own words say why.
            Err(e) => {
                stop(&mut tar);
                Err(finish(&mut server, server_err).err().unwrap_or(e))
            }
            Ok(()) => finish(&mut tar, tar_err).and_then(|()| finish(&mut server, server_err)),
        };
        if let Err(e) = result {
            stop(&mut tar);
            stop(&mut server);
            self.cleanup(&remote_partial, &local_partial);
            return Err(e);
        }
        progress(total, total);
        self.land(&local_partial, &remote_partial, &dst_dir, &name, conflict, true)
    }

    /// Brings a Cloud folder into a folder on the Mac as one tar stream. Progress counts bytes against its size on
    /// the server. There's no trash on the Mac side, so `Replace` is refused.
    pub fn download_dir(&self, rel: &str, dst_dir: &Path, conflict: Conflict, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<Option<PathBuf>> {
        let name = last_name(rel)?;
        let remote = self.remote(rel)?;
        let src = resolve(self.root, rel)?;
        if !src.is_dir() {
            return Err(Error::Invalid("Download a folder.".into()));
        }
        if dst_dir.is_dir() {
            cloud::not_inside(&src, dst_dir)?;
        }
        if conflict == Conflict::Replace {
            return Err(Error::Invalid("Replace only works inside Cloud.".into()));
        }
        if conflict == Conflict::Skip && exists(&dst_dir.join(&name)) {
            return Ok(None);
        }
        let (total, _) = self.size(rel)?;
        let local_partial = dst_dir.join(partial_name(&name));
        fs::create_dir(&local_partial)?;
        let fail = |e: Error| {
            let _ = cloud::remove(&local_partial);
            Err(e)
        };
        let mut server = match self.shell.spawn(&format!("exec tar -cf - -C {} .", quote(&remote))) {
            Ok(s) => s,
            Err(e) => return fail(e),
        };
        let server_err = drain(server.stderr.take());
        let tar = Command::new(TAR).arg("-xf").arg("-").arg("-C").arg(&local_partial).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn();
        let mut tar = match tar {
            Ok(t) => t,
            Err(e) => {
                stop(&mut server);
                return fail(e.into());
            }
        };
        let tar_err = drain(tar.stderr.take());
        let mut from = server.stdout.take().expect("stdout is piped");
        let mut to = tar.stdin.take().expect("stdin is piped");
        let pumped = pump(&mut from, &mut to, cancel, &mut |sent| progress(sent.min(total), total));
        drop(to);
        drop(from);
        let result = match pumped {
            Err(e) => {
                stop(&mut server);
                stop(&mut tar);
                Err(e)
            }
            Ok(()) => finish(&mut server, server_err).and_then(|()| finish(&mut tar, tar_err)),
        };
        if let Err(e) = result {
            stop(&mut server);
            stop(&mut tar);
            return fail(e);
        }
        progress(total, total);
        let landed = cloud::land(&local_partial, dst_dir, &name, conflict, None);
        if !matches!(landed, Ok(Some(_))) {
            let _ = cloud::remove(&local_partial);
        }
        landed
    }

    /// Copies an item inside Cloud with `cp -a`, as `name` in `dst_dir_rel`. Refused when the destination is the
    /// source folder or inside it.
    pub fn copy(&self, rel: &str, dst_dir_rel: &str, name: &str, conflict: Conflict, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<Option<PathBuf>> {
        last_name(rel)?;
        let name = plain_name(name)?;
        let src = resolve(self.root, rel)?;
        let dst_dir = resolve(self.root, dst_dir_rel)?;
        if !dst_dir.is_dir() {
            return Err(Error::Invalid("Copy into a folder.".into()));
        }
        cloud::not_inside(&src, &dst_dir)?;
        if conflict == Conflict::Skip && exists(&dst_dir.join(&name)) {
            return Ok(None);
        }
        let (total, _) = self.size(rel)?;
        let partial = partial_name(&name);
        let remote_partial = self.join(&self.remote(dst_dir_rel)?, &partial);
        let local_partial = dst_dir.join(&partial);
        let script = guarded(&format!("cp -a {} {}", quote(&self.remote(rel)?), quote(&remote_partial)));
        if let Err(e) = self.watch(&script, cancel, &mut |_| {}) {
            self.cleanup(&remote_partial, &local_partial);
            return Err(e);
        }
        progress(total, total);
        self.land(&local_partial, &remote_partial, &dst_dir, &name, conflict, true)
    }

    /// Unzips on the server into a folder beside the zip, named after it. Progress counts entries.
    pub fn extract(&self, rel: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
        let zip = self.remote(rel)?;
        fs::symlink_metadata(resolve(self.root, rel)?)?;
        let stem = Path::new(&last_name(rel)?).file_stem().map(|s| s.to_string_lossy().into_owned()).ok_or_else(|| Error::Invalid("Nothing to extract.".into()))?;
        let rel_dir = parent_rel(rel);
        // unzip reads `[`, `*` and `?` in an archive's name as a pattern, so it only ever sees a plain-named link.
        let link = self.join(&self.remote(&rel_dir)?, &partial_name("archive.zip"));
        let l = quote(&link);
        run(self.shell, &format!("ln -s -- {} {l}", quote(&zip)))?;
        let out = self.extract_link(&l, &rel_dir, &stem, cancel, progress);
        let _ = run(self.shell, &format!("rm -f -- {l}"));
        out
    }

    fn extract_link(&self, l: &str, rel_dir: &str, stem: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
        let names = run(self.shell, &format!("unzip -Z1 {l}"))?;
        if names.lines().any(leaves) {
            return Err(escapes());
        }
        let total = names.lines().filter(|l| !l.is_empty()).count() as u64;
        let dir = resolve(self.root, rel_dir)?;
        let partial = partial_name(stem);
        let remote_partial = self.join(&self.remote(rel_dir)?, &partial);
        let local_partial = dir.join(&partial);
        let p = quote(&remote_partial);
        let script = format!("mkdir {p} && {}", guarded(&format!("unzip -o {l} -d {p}")));
        let mut done = 0u64;
        let watched = self.watch(&script, cancel, &mut |l| {
            let l = l.trim_start();
            if ["inflating:", "extracting:", "creating:", "linking:"].iter().any(|k| l.starts_with(k)) {
                done += 1;
                progress(done.min(total), total);
            }
        });
        if let Err(e) = watched {
            self.cleanup(&remote_partial, &local_partial);
            return Err(e);
        }
        progress(total, total);
        self.land(&local_partial, &remote_partial, &dir, stem, Conflict::KeepBoth, false)?.ok_or_else(|| unreachable!("KeepBoth always lands"))
    }

    /// Zips items from one folder on the server, as `compress_local` names them. Progress counts items.
    pub fn compress(&self, rels: &[String], cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
        let plan = cloud::zip_plan(self.root, rels)?;
        for r in rels {
            self.remote(r)?;
        }
        let remote_dir = self.remote(&plan.rel_dir)?;
        let args = plan.items.iter().map(|p| quote(&format!("./{}", p.file_name().expect("not the root").to_string_lossy()))).collect::<Vec<_>>().join(" ");
        let count = run(self.shell, &format!("cd {} && find {args} -print0 | tr -dc '\\0' | wc -c", quote(&remote_dir)))?;
        let total = count.trim().parse::<u64>().unwrap_or(0);
        // The zip and zip's own temporary file both live in a partial folder, so a cancel leaves nothing behind.
        let partial = partial_name(&plan.name);
        let remote_partial = self.join(&remote_dir, &partial);
        let local_partial = plan.dir.join(&partial);
        let out = self.join(&remote_partial, "out.zip");
        let (p, d) = (quote(&remote_partial), quote(&remote_dir));
        let script = format!("mkdir {p} && cd {d} && {}", guarded(&format!("zip -r -y -b {p} {} {args}", quote(&out))));
        let mut done = 0u64;
        let watched = self.watch(&script, cancel, &mut |l| {
            if l.trim_start().starts_with("adding:") {
                done += 1;
                progress(done.min(total), total);
            }
        });
        if let Err(e) = watched {
            self.cleanup(&remote_partial, &local_partial);
            return Err(e);
        }
        progress(total, total);
        let landed = cloud::land(&local_partial.join("out.zip"), &plan.dir, &plan.name, Conflict::KeepBoth, None);
        self.cleanup(&remote_partial, &local_partial);
        landed?.ok_or_else(|| unreachable!("KeepBoth always lands"))
    }

    /// A move to another disk on the server: `copy` into a partial on the target disk, then the source is removed
    /// there, only once the copy has landed.
    pub fn move_across(&self, rel: &str, dest_dir_rel: &str, cancel: &AtomicBool, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf> {
        let name = last_name(rel)?;
        let source = self.remote(rel)?;
        let landed = self.copy(rel, dest_dir_rel, &name, Conflict::KeepBoth, cancel, progress)?.expect("KeepBoth always lands");
        run(self.shell, &format!("rm -rf {}", quote(&source)))?;
        Ok(landed)
    }

    /// The size and free space of the disk that holds `rel`, from `df` on the server. Through SMB the Mac only
    /// sees the share's own disk.
    pub fn space(&self, rel: &str) -> Result<cloud::Space> {
        let out = run(self.shell, &format!("df -Pk {}", quote(&self.remote(rel)?)))?;
        let line = out.lines().nth(1).ok_or_else(|| Error::Remote(format!("Couldn't read the space: {}", out.trim())))?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        let kb = |i: usize| fields.get(i).and_then(|f| f.parse::<u64>().ok()).map(|n| n * 1024);
        match (kb(1), kb(3)) {
            (Some(total), Some(free)) => Ok(cloud::Space { total, free }),
            _ => Err(Error::Remote(format!("Couldn't read the space: {}", line.trim()))),
        }
    }

    /// A file's size, or a folder's total bytes and item count, counted on the server. Links aren't followed.
    pub fn size(&self, rel: &str) -> Result<(u64, u64)> {
        let p = quote(&self.remote(rel)?);
        fs::symlink_metadata(resolve(self.root, rel)?)?;
        let script = format!(
            "if [ -d {p} ] && [ ! -L {p} ]; then \
               find {p} ! -type d -exec ls -lnd {{}} + | awk '{{s+=$5}} END {{print s+0}}'; \
               find {p} -mindepth 1 -print0 | tr -dc '\\0' | wc -c; \
             else ls -lnd {p} | awk '{{print $5}}'; echo 1; fi"
        );
        let out = run(self.shell, &script)?;
        let mut nums = out.split_whitespace().map(|n| n.parse::<u64>());
        match (nums.next(), nums.next()) {
            (Some(Ok(bytes)), Some(Ok(items))) => Ok((bytes, items)),
            _ => Err(Error::Remote(format!("Couldn't read the size: {}", out.trim()))),
        }
    }
}
