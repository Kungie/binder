//! What Binder does while it runs, window open or not: the agent picks up its tasks every 10 minutes, and the
//! server's containers are watched every minute, with a Mac notification when one is in trouble.

use crate::AppState;
use binder_core::remote::SshRemote;
use binder_core::watch::Watch;
use binder_core::{night, pickup, server};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const PICK_UP_FIRST: Duration = Duration::from_secs(30);
const PICK_UP_EVERY: Duration = Duration::from_secs(600);
const WATCH_EVERY: Duration = Duration::from_secs(60);

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

/// One pick-up round; the interface reloads tasks when anything was read or started.
fn pick_up(app: &AppHandle) {
    let (Some(target), on, _, _) = settings(app) else { return };
    let remote = SshRemote::new(target);
    let night = night::cron_state(&remote).unwrap_or(false);
    let state = app.state::<AppState>();
    if let Ok(round) = pickup::round(&state.db, &remote, now(), on, night) {
        if round.polled > 0 || round.started.is_some() || night {
            let _ = app.emit("tasks://changed", ());
        }
    }
}

/// One container check. Turning the switch off forgets what was seen, so turning it on starts fresh.
fn look(app: &AppHandle, watch: &mut Watch) {
    let (target, _, on, sd) = settings(app);
    let (Some(target), true) = (target, on) else {
        *watch = Watch::default();
        return;
    };
    let alerts = match server::fetch(&SshRemote::new(target), now(), Some(&sd)) {
        Ok(status) => watch.seen(&status.containers),
        Err(_) => watch.missed(),
    };
    for alert in alerts {
        let _ = app.notification().builder().title(alert.title()).body(alert.body()).show();
        let _ = app.emit("watch://alert", alert.title());
    }
}
