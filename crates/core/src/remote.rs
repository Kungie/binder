//! Running a script on the home server over SSH. Everything Binder does there goes through `Remote`,
//! so tests can stand in a `FakeRemote`.

use crate::{Error, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};

pub trait Remote {
    /// Runs `script` with `sh` on the server. `stdin` is piped to the script after it.
    fn run(&self, script: &str, stdin: Option<&str>) -> Result<String>;
}

/// The system's `ssh`, non-interactive: keys only, a 5 second connect timeout, keepalives, and a deadline for the
/// whole call so a stalled connection can't hang Binder.
pub struct SshRemote {
    pub target: String,
    /// `ssh`; tests put a stand-in here.
    pub program: String,
    pub deadline: Duration,
}

impl SshRemote {
    pub fn new(target: impl Into<String>) -> SshRemote {
        SshRemote { target: target.into(), program: "ssh".into(), deadline: Duration::from_secs(20) }
    }
}

impl Remote for SshRemote {
    fn run(&self, script: &str, stdin: Option<&str>) -> Result<String> {
        // The remote shell reads the script on its stdin. Data for the script's own stdin travels base64-encoded,
        // so nothing in it can end the data early and run as a command.
        let body = match stdin {
            Some(data) => format!("printf %s '{}' | base64 -d | sh -c {}\n", STANDARD.encode(data), quote(script)),
            None => script.to_string(),
        };
        let mut child = Command::new(&self.program)
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2"])
            .args(["--", &self.target, "sh", "-l", "-s"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Remote(format!("Couldn't start ssh: {e}")))?;
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut p) = pipe {
                    let _ = p.read_to_end(&mut buf);
                }
                buf
            })
        };
        let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        if let Some(mut input) = child.stdin.take() {
            let _ = input.write_all(body.as_bytes());
        }
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if started.elapsed() > self.deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Remote(format!("{} didn't answer in time", self.target)));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
        if status.success() {
            Ok(String::from_utf8_lossy(&out).into_owned())
        } else {
            let err = String::from_utf8_lossy(&err).trim().chars().take(300).collect::<String>();
            Err(Error::Remote(if err.is_empty() { format!("Can't reach {}", self.target) } else { err }))
        }
    }
}

/// POSIX single-quoting: the text reaches the shell as one word, never as code.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Replies in order and remembers what was sent. For tests.
#[doc(hidden)]
#[derive(Default)]
pub struct FakeRemote {
    pub replies: std::cell::RefCell<std::collections::VecDeque<Result<String>>>,
    pub seen: std::cell::RefCell<Vec<(String, Option<String>)>>,
}

impl FakeRemote {
    pub fn new(replies: Vec<Result<String>>) -> FakeRemote {
        FakeRemote { replies: std::cell::RefCell::new(replies.into()), seen: Default::default() }
    }
}

impl Remote for FakeRemote {
    fn run(&self, script: &str, stdin: Option<&str>) -> Result<String> {
        self.seen.borrow_mut().push((script.to_string(), stdin.map(String::from)));
        self.replies.borrow_mut().pop_front().unwrap_or_else(|| Ok(String::new()))
    }
}
