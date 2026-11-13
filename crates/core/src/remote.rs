//! Running a script on the home server over SSH. Everything Binder does there goes through `Remote`,
//! so tests can stand in a `FakeRemote`.

use crate::{Error, Result};
use std::io::Write;
use std::process::{Command, Stdio};

pub trait Remote {
    /// Runs `script` with `sh` on the server. `stdin` is piped to the script after it.
    fn run(&self, script: &str, stdin: Option<&str>) -> Result<String>;
}

/// The system's `ssh`, non-interactive: keys only, a 5 second connect timeout.
pub struct SshRemote {
    pub target: String,
}

/// A here-document marker the script and its input are split on.
const SPLIT: &str = "BINDER_STDIN_9f2c";

impl Remote for SshRemote {
    fn run(&self, script: &str, stdin: Option<&str>) -> Result<String> {
        // The remote shell reads the script from stdin; data meant for the script's own stdin rides in a here-document.
        let body = match stdin {
            Some(data) => format!("sh -c {} <<'{SPLIT}'\n{data}\n{SPLIT}\n", quote(script)),
            None => script.to_string(),
        };
        let mut child = Command::new("ssh")
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", &self.target, "sh", "-l", "-s"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Remote(format!("Couldn't start ssh: {e}")))?;
        child.stdin.take().expect("piped").write_all(body.as_bytes())?;
        let out = child.wait_with_output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().chars().take(300).collect::<String>();
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
