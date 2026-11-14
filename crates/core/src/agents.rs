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
/// (tmux applies `history-limit` only to new panes) and the session's own look set after. `-u` makes tmux send
/// UTF-8 whatever locale the server gives a session from an app opened in Finder, which has no `LANG`.
pub fn remote_command(agent: Agent) -> String {
    let session = agent.session();
    let inner = format!("bash -lc {}", quote(agent.program()));
    format!(
        "tmux -u start-server \\; set -g history-limit 10000 \\; new-session -A -s {session} {} \\; set-option -t {session} status off \\; set-option -t {session} mouse on \\; set-option -t {session} set-clipboard on",
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
            .split(['\r', '\n'])
            .map(plain)
            .filter(|l| !l.is_empty())
            .last()
            .unwrap_or_else(|| "Couldn't reach the server.".into()),
        Some(0) => "Detached.".into(),
        None => "Disconnected.".into(),
        Some(_) => "Session ended.".into(),
    }
}

/// What a line of terminal output says after its last escape sequence, without control characters. When a session
/// that was showing drops, the line holds tmux's last redraw and then `ssh`'s own message.
fn plain(line: &str) -> String {
    let mut after = 0;
    let mut chars = line.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c != '\x1b' {
            continue;
        }
        match chars.next() {
            // CSI: parameters, then one final byte in @..~.
            Some((_, '[')) => {
                for (_, c) in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or ESC \.
            Some((_, ']')) => {
                while let Some((_, c)) = chars.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' {
                        chars.next();
                        break;
                    }
                }
            }
            // Character set selection takes one more character.
            Some((_, '(' | ')' | '*' | '+')) => {
                chars.next();
            }
            _ => {}
        }
        after = chars.peek().map_or(line.len(), |&(i, _)| i);
    }
    line[after..].chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string()
}
