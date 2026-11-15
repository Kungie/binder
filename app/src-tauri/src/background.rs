//! What Binder does while it runs, window open or not: the agent picks up its tasks every 10 minutes, and the
//! server's containers are watched every minute, with a Mac notification when one is in trouble.

use crate::AppState;
use binder_core::remote::SshRemote;
use binder_core::events::EventKind;
use binder_core::watch::{self, Watch};
use binder_core::{night, pickup, server};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const PICK_UP_FIRST: Duration = Duration::from_secs(30);
const PICK_UP_EVERY: Duration = Duration::from_secs(600);
const WATCH_EVERY: Duration = Duration::from_secs(60);
/// More than this since the last good check, and the server's own times fill the gap.
const GAP: i64 = 300;
/// Events are kept five weeks, for the weekly note.
const KEEP_EVENTS: i64 = 35 * 86_400;
/// An owner's restart keeps that container quiet this long.
const OWNER_QUIET: i64 = 120;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The server target when one is set, and the settings a round needs.
fn settings(app: &AppHandle) -> (Option<String>, bool, bool, String) {
    let state = app.state::<AppState>();
    let c = state.config.lock().unwrap();
    let sd = format!("{}/SD", c.cloud_remote.trim_end_matches('/'));
    (c.server.clone().filter(|s| !s.trim().is_empty()), c.pick_up, c.watch, sd)
}

pub fn start(app: &AppHandle) {
    let a = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(PICK_UP_FIRST);
        loop {
            pick_up(&a);
            std::thread::sleep(PICK_UP_EVERY);
        }
    });
    let a = app.clone();
    std::thread::spawn(move || {
        let mut watch = Watch::default();
        loop {
            look(&a, &mut watch);
            std::thread::sleep(WATCH_EVERY);
        }
    });
}

/// Writes the weekly notes that are due, when the vault is there.
fn weekly(app: &AppHandle) {
    use chrono::Offset;
    let state = app.state::<AppState>();
    let vault = state.config.lock().unwrap().vault.clone();
    let now = chrono::Local::now();
    let _ = binder_core::weekly::write_due(&mut state.db.lock().unwrap(), &vault, now.date_naive(), now.offset().fix());
}

/// One pick-up round; the interface reloads tasks when anything was read or started.
fn pick_up(app: &AppHandle) {
    weekly(app);
    let (Some(target), on, _, _) = settings(app) else { return };
    let remote = SshRemote::new(target);
    let night = night::cron_state(&remote).unwrap_or(false);
    let state = app.state::<AppState>();
    let _ = state.db.lock().unwrap().prune_events(now() - KEEP_EVENTS);
    if let Ok(round) = pickup::round(&state.db, &remote, now(), on, night) {
        if round.polled > 0 || round.started.is_some() || night {
            let _ = app.emit("tasks://changed", ());
        }
    }
}

/// One container check. Everything it finds is logged for the brief whenever a server is set; notifications go out
/// only with the switch on. Without a server the watch forgets what it saw, so setting one starts fresh.
fn look(app: &AppHandle, watch: &mut Watch) {
    let (target, _, on, sd) = settings(app);
    let Some(target) = target else {
        *watch = Watch::default();
        return;
    };
    let remote = SshRemote::new(target);
    let now = now();
    let state = app.state::<AppState>();
    let (quiet, last, boot) = {
        let db = state.db.lock().unwrap();
        let owner: Vec<(String, i64)> = db
            .events_between(now - OWNER_QUIET, now + 1)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.kind == EventKind::OwnerRestart)
            .map(|e| (e.subject, e.at))
            .collect();
        let meta = |k: &str| db.meta(k).ok().flatten().and_then(|v| v.parse::<i64>().ok());
        (owner, meta("watch_last"), meta("server_boot"))
    };
    let quiet_names: Vec<String> = quiet.iter().map(|(n, _)| n.clone()).collect();
    let mut logged: Vec<(i64, EventKind, String, String)> = vec![];
    let alerts = match server::fetch(&remote, now, Some(&sd)) {
        // A failed `docker ps` says nothing about the containers: skip it rather than call them all removed.
        Ok(status) if status.docker_failed => vec![],
        Ok(status) => {
            if let Some(at) = watch::rebooted(boot, status.boot_at) {
                logged.push((at, EventKind::ServerRestart, String::new(), String::new()));
            }
            // The Mac slept or Binder was closed: read what the server remembers about the gap.
            if let Some(last) = last.filter(|l| now - l > GAP) {
                if let Ok(times) = server::container_times(&remote) {
                    for (at, kind, name) in watch::catch_up(last, now, &times, &quiet) {
                        logged.push((at, kind, name, String::new()));
                    }
                }
            }
            let mut db = state.db.lock().unwrap();
            let _ = db.set_meta("watch_last", &now.to_string());
            if let Some(b) = status.boot_at {
                let _ = db.set_meta("server_boot", &b.to_string());
            }
            drop(db);
            watch.seen_with(&status.containers, &quiet_names)
        }
        Err(_) => watch.missed(),
    };
    for alert in &alerts {
        if let Some(kind) = alert.kind() {
            logged.push((now, kind, alert.subject().to_string(), alert.body()));
        }
    }
    if !logged.is_empty() {
        let mut db = state.db.lock().unwrap();
        for (at, kind, subject, detail) in &logged {
            let _ = db.log_event(*at, *kind, subject, detail);
        }
        drop(db);
        let _ = app.emit("brief://changed", ());
    }
    if !on {
        return;
    }
    for alert in alerts {
        let _ = app.notification().builder().title(alert.title()).body(alert.body()).show();
        let _ = app.emit("watch://alert", alert.title());
    }
}
