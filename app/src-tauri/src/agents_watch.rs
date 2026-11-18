//! Watches the Agents' tmux sessions on the server: every 5 s while the Agents screen shows, every 20 s otherwise.
//! Each read goes to the window as `agents://sessions`; a session whose work just stopped sends one notification,
//! unless Binder is in front showing that session.

use crate::AppState;
use binder_core::agents::{self, State};
use binder_core::remote::{Remote, SshRemote};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// The session on screen, while the Agents screen shows.
static VIEWING: Mutex<Option<Option<String>>> = Mutex::new(None);

pub fn viewing(session: Option<String>) {
    *VIEWING.lock().unwrap() = session.map(Some);
}

fn shown() -> bool {
    VIEWING.lock().unwrap().is_some()
}

/// A session's name for people: the name the owner gave it, "Claude Code", "Ollama", or "Claude Code · <project>".
fn label(app: &AppHandle, session: &str) -> String {
    let state = app.state::<AppState>();
    let given = state.config.lock().ok().and_then(|c| c.agent_names.get(session).cloned().or_else(|| c.agents.iter().find(|a| format!("binder-claude-{}", agents::slug(&a.name)) == session).map(|a| a.name.clone())));
    if let Some(name) = given {
        return name;
    }
    match session {
        "binder-claude" => "Claude Code".into(),
        "binder-ollama" => "Ollama".into(),
        s => {
            let slug = s.strip_prefix("binder-claude-").unwrap_or(s);
            let name = state.db.lock().ok().and_then(|db| db.list_projects().ok()).and_then(|ps| ps.into_iter().find(|p| agents::slug(&p.name) == slug).map(|p| p.name));
            format!("Claude Code · {}", name.unwrap_or_else(|| slug.to_string()))
        }
    }
}

pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut seen: HashMap<String, State> = HashMap::new();
        loop {
            round(&app, &mut seen);
            // Wakes early when the screen opens, so its first read isn't 20 s away.
            for _ in 0..if shown() { 5 } else { 20 } {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

fn round(app: &AppHandle, seen: &mut HashMap<String, State>) {
    let target = app.state::<AppState>().config.lock().unwrap().server.clone().filter(|s| !s.trim().is_empty());
    let Some(target) = target else { return };
    let Ok(out) = SshRemote::new(target).run(agents::sessions_script(), None) else { return };
    let sessions = agents::parse_sessions(&out);
    let _ = app.emit("agents://sessions", &sessions);
    let focused = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
    let on_screen = VIEWING.lock().unwrap().clone().flatten();
    for (session, state) in agents::alerts(seen, &sessions) {
        if focused && on_screen.as_deref() == Some(session.as_str()) {
            continue;
        }
        let body = if state == State::Waiting { "Waiting for you." } else { "Finished." };
        let _ = app.notification().builder().title(label(app, &session)).body(body).sound("default").show();
    }
}
