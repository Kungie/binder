//! Running a shell script on the home server, with its input and output piped, for work that streams: the Cloud
//! fast path. `remote.rs` stays the way for short commands.

use crate::remote::quote;
use crate::{Error, Result};
use std::process::{Child, Command, Stdio};

pub trait Shell: Send + Sync {
    /// Starts `script` with stdin, stdout and stderr piped.
    fn spawn(&self, script: &str) -> Result<Child>;
}

/// `ssh` to the server, never touching `~/.ssh`: the owner's own config and agent do the rest.
pub struct SshShell {
    pub target: String,
    /// `ssh`, or a full path to it.
    pub program: String,
}

impl Shell for SshShell {
    fn spawn(&self, script: &str) -> Result<Child> {
        Ok(Command::new(&self.program)
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2", "--"])
            .arg(&self.target)
            .args(["sh", "-c"])
            // ssh joins its arguments into one line for the remote shell, so the script is quoted once more.
            .arg(quote(script))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?)
    }
}

/// `sh` on this Mac, standing in for the server in tests.
pub struct LocalShell;

impl Shell for LocalShell {
    fn spawn(&self, script: &str) -> Result<Child> {
        Ok(Command::new("sh").arg("-c").arg(script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?)
    }
}

/// Runs a script to the end: its stdout, or its stderr as the error.
pub fn run(shell: &dyn Shell, script: &str) -> Result<String> {
    let mut child = shell.spawn(script)?;
    drop(child.stdin.take());
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(Error::Remote(if err.is_empty() { format!("The server said {}.", out.status) } else { err }))
    }
}
