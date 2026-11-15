//! Keeps the vault share mounted. Binder uses macOS's own `mount volume`, which takes the password saved in the
//! Keychain, so nothing asks. A share whose password prompt is canceled isn't tried again until Connect; one that
//! doesn't answer (the Mac is away from home) is tried again every 10 minutes.

use binder_core::config::mountable;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_secs(600);

enum Tried {
    At(Instant),
    Canceled,
}

static TRIED: Mutex<BTreeMap<String, Tried>> = Mutex::new(BTreeMap::new());

#[derive(Debug, PartialEq)]
pub enum Failed {
    Address,
    Canceled,
    NoAnswer,
}

impl Failed {
    pub fn text(&self) -> &'static str {
        match self {
            Failed::Address => "The vault share's address starts with smb://.",
            Failed::Canceled => "Connecting to the vault share was canceled.",
            Failed::NoAnswer => "The vault share didn't answer. Is the Mac on the home network?",
        }
    }
}

/// Mounts one share and waits for it, at most 30 seconds.
pub fn mount(share: &str) -> Result<(), Failed> {
    if !mountable(share) {
        return Err(Failed::Address);
    }
    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!("mount volume \"{share}\""))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| Failed::NoAnswer)?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() < TIMEOUT => std::thread::sleep(Duration::from_millis(200)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failed::NoAnswer);
            }
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut err = String::new();
    let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut err));
    // AppleScript's "User canceled." is error -128.
    Err(if err.contains("-128") { Failed::Canceled } else { Failed::NoAnswer })
}

/// Connect, pressed: forgets an earlier cancel and mounts now.
pub fn connect(share: &str) -> Result<(), Failed> {
    TRIED.lock().unwrap_or_else(|e| e.into_inner()).remove(share);
    let r = mount(share);
    note(share, &r);
    r
}

fn note(share: &str, r: &Result<(), Failed>) {
    let mut tried = TRIED.lock().unwrap_or_else(|e| e.into_inner());
    match r {
        Ok(()) => tried.remove(share),
        Err(Failed::Canceled) => tried.insert(share.to_string(), Tried::Canceled),
        Err(_) => tried.insert(share.to_string(), Tried::At(Instant::now())),
    };
}

/// One background look: mounts the share when its folder is gone, unless a try was canceled or is too recent.
pub fn keep(here: bool, share: Option<String>) {
    let Some(share) = share else { return };
    if here {
        TRIED.lock().unwrap_or_else(|e| e.into_inner()).remove(&share);
        return;
    }
    let due = match TRIED.lock().unwrap_or_else(|e| e.into_inner()).get(&share) {
        Some(Tried::Canceled) => false,
        Some(Tried::At(t)) => t.elapsed() >= RETRY,
        None => true,
    };
    if due {
        let r = mount(&share);
        note(&share, &r);
    }
}

/// Whether `path` is a share mounted there, not a folder left on the Mac's own disk.
pub fn here(path: &Path) -> bool {
    crate::commands::vault_here(path)
}
