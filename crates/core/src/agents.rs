//! The Agents screen's chats: tmux sessions on the home server, reached with the Mac's own `ssh -tt`.
//! Binder only builds the command lines here; the app runs them in a pseudo-terminal.

use crate::agent::shell_dir;
use crate::remote::quote;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One terminal on the Agents screen: Claude Code at home or in a folder, Ollama with a model, or any other
/// `binder-*` session found running on the server.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Pane {
    Claude { project: Option<String>, dir: Option<String> },
    Ollama { model: String },
    Attach { session: String },
}

/// The model Ollama runs until the owner picks another.
pub const DEFAULT_MODEL: &str = "qwen3:4b";

/// A project's name as part of a tmux session name: lowercase letters, digits and single dashes.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() { "project".into() } else { out.into() }
}

/// A model name Ollama could know: letters, digits and `._:/-`.
pub fn valid_model(model: &str) -> bool {
    !model.is_empty() && model.len() <= 200 && model.chars().all(|c| c.is_ascii_alphanumeric() || "._:/-".contains(c))
}

/// A tmux session of Binder's.
pub fn valid_session(session: &str) -> bool {
    session.strip_prefix("binder-").is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

impl Pane {
    /// The tmux session on the server. Fixed per pane, so every attach finds the same chat.
    pub fn session(&self) -> String {
        match self {
            Pane::Claude { project: None, .. } => "binder-claude".into(),
            Pane::Claude { project: Some(p), .. } => format!("binder-claude-{}", slug(p)),
            Pane::Ollama { .. } => "binder-ollama".into(),
            Pane::Attach { session } => session.clone(),
        }
    }

    /// What runs inside the session. A login shell stays after it exits, so the session survives.
    fn program(&self) -> Result<String> {
        match self {
            // The folder is made if it isn't there yet, so a new project's session starts in it.
            Pane::Claude { dir: Some(d), .. } if !d.trim().is_empty() => {
                let d = shell_dir(d);
                Ok(format!("mkdir -p {d} && cd {d} && claude; exec bash -l"))
            }
            Pane::Claude { .. } => Ok("claude; exec bash -l".into()),
            Pane::Ollama { model } if valid_model(model) => Ok(format!("docker exec -it ollama ollama run {model}; exec bash -l")),
            Pane::Ollama { .. } => Err(Error::Invalid("That isn't a model name.".into())),
            Pane::Attach { session } if valid_session(session) => Ok("exec bash -l".into()),
            Pane::Attach { .. } => Err(Error::Invalid("That isn't one of Binder's sessions.".into())),
        }
    }
}

/// The command the server's login shell runs: create the session or attach to it, with scrollback set first
/// (tmux applies `history-limit` only to new panes) and the session's own look set after. `-u` makes tmux send
/// UTF-8 whatever locale the server gives a session from an app opened in Finder, which has no `LANG`.
pub fn remote_command(pane: &Pane) -> Result<String> {
    let program = pane.program()?;
    let session = pane.session();
    let inner = format!("bash -lc {}", quote(&program));
    Ok(format!(
        "tmux -u start-server \\; set -g history-limit 10000 \\; new-session -A -s {session} {} \\; set-option -t {session} status off \\; set-option -t {session} mouse on \\; set-option -t {session} set-clipboard on",
        quote(&inner)
    ))
}

/// `ssh`'s arguments: a forced terminal, keys only, keepalives, then the target as one argument.
pub fn ssh_args(target: &str, pane: &Pane) -> Result<Vec<String>> {
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
    args.push(remote_command(pane)?);
    Ok(args)
}

/// What a session is doing, as its screen shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Working,
    /// Claude Code asks the owner something.
    Waiting,
    Idle,
    /// The program ended and the session's shell is left.
    Exited,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub session: String,
    pub state: State,
    /// The screen's last line with text.
    pub line: String,
}

const SHELLS: [&str; 4] = ["bash", "sh", "zsh", "fish"];

/// A session's state from its foreground command and visible screen, and the screen's last line with text.
pub fn classify(command: &str, screen: &str) -> (State, String) {
    let line = screen.lines().map(str::trim).filter(|l| !l.is_empty()).last().unwrap_or("").to_string();
    let state = if SHELLS.contains(&command) {
        State::Exited
    } else if screen.contains("Do you want") || screen.contains("\u{276f} 1.") {
        State::Waiting
    } else if screen.contains("esc to interrupt") {
        State::Working
    } else {
        State::Idle
    };
    (state, line)
}

const MARK: &str = "--binder:pane-- ";

/// Every `binder-*` session's program and visible screen, in one call. tmux reports the pane's shell, which runs
/// the program as its child, so the child's name is read first; without one, the shell is all that is left.
pub fn sessions_script() -> &'static str {
    "tmux list-sessions -F '#{session_name}' 2>/dev/null | grep '^binder-' | while read -r s; do\n\
     p=$(tmux display-message -p -t \"=$s:\" '#{pane_pid}')\n\
     c=$(ps -o comm= --ppid \"$p\" 2>/dev/null | head -n 1)\n\
     [ -n \"$c\" ] || c=$(tmux display-message -p -t \"=$s:\" '#{pane_current_command}')\n\
     printf '\\n--binder:pane-- %s %s\\n' \"$s\" \"$c\"\n\
     tmux capture-pane -p -t \"=$s:\"\n\
     done\n"
}

pub fn parse_sessions(out: &str) -> Vec<Session> {
    let mut found = vec![];
    for part in out.split(MARK).skip(1) {
        let (head, screen) = part.split_once('\n').unwrap_or((part, ""));
        let mut words = head.split_whitespace();
        let Some(session) = words.next().filter(|s| valid_session(s)) else { continue };
        let (state, line) = classify(words.next().unwrap_or(""), screen);
        found.push(Session { session: session.into(), state, line });
    }
    found
}

/// Sessions whose work just stopped: Working before, Waiting or Idle now. `seen` keeps each session's last state.
pub fn alerts(seen: &mut HashMap<String, State>, now: &[Session]) -> Vec<(String, State)> {
    let mut out = vec![];
    for s in now {
        let before = seen.insert(s.session.clone(), s.state);
        if before == Some(State::Working) && matches!(s.state, State::Waiting | State::Idle) {
            out.push((s.session.clone(), s.state));
        }
    }
    seen.retain(|k, _| now.iter().any(|s| &s.session == k));
    out
}

/// Ends one of Binder's tmux sessions on the server.
pub fn close_script(session: &str) -> Result<String> {
    if !valid_session(session) {
        return Err(Error::Invalid("That isn't one of Binder's sessions.".into()));
    }
    Ok(format!("tmux kill-session -t {}", quote(&format!("={session}"))))
}

pub const MODELS_SCRIPT: &str = "docker exec ollama ollama list\n";

/// The model names from `ollama list`, header left out.
pub fn parse_models(out: &str) -> Vec<String> {
    out.lines().skip(1).filter_map(|l| l.split_whitespace().next()).filter(|m| valid_model(m)).map(String::from).collect()
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
