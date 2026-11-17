//! What Binder does while it runs, window open or not: the agent picks up its tasks every 10 minutes, and the
//! server's containers are watched every minute, with a Mac notification when one is in trouble, and the vault
//! share is mounted again when it dropped.

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
/// How often the vault share is checked and mounted again when it dropped (after sleep, say).
const SHARES_EVERY: Duration = Duration::from_secs(60);
const REMIND_EVERY: Duration = Duration::from_secs(30);
const SITES_EVERY: Duration = Duration::from_secs(300);
/// Events are kept five weeks, for the weekly note.
const KEEP_EVENTS: i64 = 35 * 86_400;

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
    std::thread::spawn(move || loop {
        remind(&a);
        std::thread::sleep(REMIND_EVERY);
    });
    let a = app.clone();
    std::thread::spawn(move || {
        let mut watch = binder_core::uptime::Watch::default();
        let mut known = (0, vec![]);
        std::thread::sleep(PICK_UP_FIRST);
        loop {
            check_sites(&a, &mut watch, &mut known);
            std::thread::sleep(SITES_EVERY);
        }
    });
    let a = app.clone();
    std::thread::spawn(move || loop {
        let (path, share) = {
            let state = a.state::<AppState>();
            let c = state.config.lock().unwrap();
            (c.vault.clone(), c.vault_share_address())
        };
        crate::shares::keep(crate::shares::here(&path), share);
        std::thread::sleep(SHARES_EVERY);
    });
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
    // Checked before the database lock: a stalled share would otherwise hold every command up.
    if !crate::commands::vault_here(&vault) {
        return;
    }
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
    let (last, boot) = {
        let db = state.db.lock().unwrap();
        let meta = |k: &str| db.meta(k).ok().flatten().and_then(|v| v.parse::<i64>().ok());
        (meta("watch_last"), meta("server_boot"))
    };
    let fetched = server::fetch(&remote, now, Some(&sd));
    // Read after the fetch, so a restart pressed while it ran still counts as the owner's.
    let quiet: Vec<(String, i64)> = state
        .db
        .lock()
        .unwrap()
        .events_between(watch::owner_since(last, now), self::now() + 1)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.kind == EventKind::OwnerRestart)
        .map(|e| (e.subject, e.at))
        .collect();
    let quiet_names: Vec<String> = quiet.iter().map(|(n, _)| n.clone()).collect();
    let mut logged: Vec<(i64, EventKind, String, String)> = vec![];
    let mut caught: Vec<(i64, EventKind, String)> = vec![];
    let mut full: Vec<(String, u32)> = vec![];
    let alerts = match fetched {
        // A failed `docker ps` says nothing about the containers: skip it rather than call them all removed.
        Ok(status) if status.docker_failed => vec![],
        Ok(status) => {
            if let Some(at) = watch::rebooted(boot, status.boot_at) {
                logged.push((at, EventKind::ServerRestart, String::new(), String::new()));
            }
            // The Mac slept or Binder was closed: read what the server remembers about the gap. A failed read
            // keeps the gap open for the next check.
            let mut read = true;
            if let Some(last) = last.filter(|l| now - l > watch::GAP) {
                match server::container_times(&remote) {
                    Ok(times) => {
                        caught = watch::catch_up(last, now, &times, &quiet);
                        logged.extend(caught.iter().map(|(at, kind, name)| (*at, *kind, name.clone(), String::new())));
                    }
                    Err(_) => read = false,
                }
            }
            let mut db = state.db.lock().unwrap();
            if read {
                let _ = db.set_meta("watch_last", &now.to_string());
            }
            if let Some(b) = status.boot_at {
                let _ = db.set_meta("server_boot", &b.to_string());
            }
            // Disks past 90 %, each once until it drops below 85 %; remembered across starts.
            let mut warned: std::collections::BTreeSet<String> =
                db.meta("disk_warned").ok().flatten().unwrap_or_default().split('|').filter(|s| !s.is_empty()).map(String::from).collect();
            let disks = [("Server disk", status.disk_size, status.disk_used), ("SD card", status.sd_size.unwrap_or(0), status.sd_used.unwrap_or(0))];
            full = watch::disk_warnings(&disks, &mut warned);
            let _ = db.set_meta("disk_warned", &warned.into_iter().collect::<Vec<_>>().join("|"));
            drop(db);
            logged.extend(full.iter().map(|(name, pct)| (now, EventKind::DiskFull, name.clone(), pct.to_string())));
            watch.seen_with(&status.containers, &quiet_names)
        }
        Err(_) => watch.missed(),
    };
    logged.extend(watch::live_events(&alerts, &caught, now));
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
    for (name, pct) in full {
        let _ = app.notification().builder().title(format!("{name} is {pct}% full")).body("Free some space before it fills up.").show();
    }
}

/// Every 30 seconds: a notification for each open task whose due time has come.
fn remind(app: &AppHandle) {
    let state = app.state::<AppState>();
    let due = state.db.lock().unwrap().due_reminders(chrono::Local::now().naive_local()).unwrap_or_default();
    for t in due {
        let when = t.due_time.clone().unwrap_or_default();
        let _ = app.notification().builder().title(&t.title).body(format!("Due at {when}")).sound("default").show();
    }
}

/// Every 5 minutes: each site is asked from the Mac, kept a week for its uptime bars, and two failures in a row
/// make it down.
fn check_sites(app: &AppHandle, watch: &mut binder_core::uptime::Watch, known: &mut (i64, Vec<(String, String)>)) {
    let (Some(target), _, on, _) = settings(app) else { return };
    let state = app.state::<AppState>();
    let now = now();
    // The list of sites costs the server a `docker stats`; it is read again every 30 minutes, not every check.
    if known.1.is_empty() || now - known.0 >= 1800 {
        let saved = state.config.lock().unwrap().site_addresses.clone();
        let Ok(sites) = binder_core::sites::read(&SshRemote::new(target), &saved) else { return };
        *known = (now, sites.into_iter().filter_map(|s| Some((s.name, s.address?))).collect());
    }
    let mut news = vec![];
    for (name, url) in &known.1 {
        let Ok((check, ms)) = crate::commands::check_site(url) else { continue };
        let ok = check.error.is_none() && check.status.is_some_and(|c| c < 500);
        let _ = binder_core::uptime::record(&mut state.db.lock().unwrap(), name, now, ok, ms);
        if let Some(kind) = watch.saw(name, ok) {
            news.push((kind, name.clone()));
        }
    }
    let mut db = state.db.lock().unwrap();
    let _ = binder_core::uptime::prune(&mut db, now - 8 * 86_400);
    for (kind, name) in &news {
        let _ = db.log_event(now, *kind, name, "");
    }
    drop(db);
    if news.is_empty() {
        return;
    }
    let _ = app.emit("brief://changed", ());
    if on {
        for (kind, name) in news {
            let (title, body) = if kind == EventKind::SiteDown { (format!("{name} is down"), "It didn't answer twice in a row.") } else { (format!("{name} is up again"), "It answers again.") };
            let _ = app.notification().builder().title(title).body(body).show();
        }
    }
}
