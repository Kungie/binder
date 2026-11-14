//! The Agents screen's two chats: tmux sessions on the home server, reached with the Mac's own `ssh -tt`.
//! Binder only builds the command lines here; the app runs them in a pseudo-terminal.

use crate::remote::quote;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Qwen,
}

impl Agent {
    /// The tmux session on the server. Fixed, so every attach finds the same chat.
    pub fn session(self) -> &'static str {
        match self {
            Agent::Claude => "binder-claude",
            Agent::Qwen => "binder-qwen",
        }
    }

    /// What runs inside the session. A login shell stays after it exits, so the session survives.
    fn program(self) -> &'static str {
        match self {
            Agent::Claude => "claude; exec bash -l",
            Agent::Qwen => "docker exec -it ollama ollama run qwen3:4b; exec bash -l",
        }
    }
}

/// The command the server's login shell runs: create the session or attach to it, with scrollback set first
/// (tmux applies `history-limit` only to new panes) and the session's own look set after.
pub fn remote_command(agent: Agent) -> String {
    let session = agent.session();
    let inner = format!("bash -lc {}", quote(agent.program()));
    format!(
        "tmux start-server \\; set -g history-limit 10000 \\; new-session -A -s {session} {} \\; set-option -t {session} status off \\; set-option -t {session} mouse on \\; set-option -t {session} set-clipboard on",
        quote(&inner)
    )
}

/// `ssh`'s arguments: a forced terminal, keys only, keepalives, then the target as one argument.
pub fn ssh_args(target: &str, agent: Agent) -> Vec<String> {
    let mut args: Vec<String> = [
        "-tt",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=5",
        "-o",
        "ServerAliveInterval=15",
        "-o",
        "ServerAliveCountMax=2",
        "--",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    args.push(target.to_string());
    args.push(remote_command(agent));
    args
}

/// One line for a panel whose `ssh` ended: why, as far as the exit code and the last output tell.
pub fn exit_message(code: Option<i32>, last_output: &str) -> String {
    match code {
        Some(127) => "tmux isn't installed on the server.".into(),
        Some(255) => last_output
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or("Couldn't reach the server.")
            .to_string(),
        Some(0) => "Detached.".into(),
        None => "Disconnected.".into(),
        Some(_) => "Session ended.".into(),
    }
}
