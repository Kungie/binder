//! The tailnet from the Mac's Tailscale app: status, pings every 5 minutes kept for a day, notifications when a
//! computer goes offline or comes back and when a key is about to expire, and Taildrop.

use crate::AppState;
use binder_core::tailscale::{self, Ping, Status};
use std::collections::{HashMap, VecDeque};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// One ping: when, how long, and which way.
#[derive(Clone, serde::Serialize)]
pub struct Sample {
    pub at: i64,
    pub ms: Option<f64>,
    pub via: String,
}

/// Each device's pings of the last 24 hours, by Tailscale id. Kept in memory only.
static HISTORY: Mutex<Option<HashMap<String, VecDeque<Sample>>>> = Mutex::new(None);

const DAY: i64 = 86_400;

/// The Tailscale command line: the app's own binary, or one on the usual paths.
fn cli() -> Option<&'static str> {
    ["/Applications/Tailscale.app/Contents/MacOS/Tailscale", "/opt/homebrew/bin/tailscale", "/usr/local/bin/tailscale"].into_iter().find(|p| std::path::Path::new(p).exists())
}

/// The app's binary runs as the command line only when told to; started from another app it tries to open its
/// window instead and prints nothing.
fn command() -> Result<Command, String> {
    let mut c = Command::new(cli().ok_or("Tailscale isn't installed on this Mac.")?);
    c.env("TAILSCALE_BE_CLI", "1").stdin(std::process::Stdio::null());
    Ok(c)
}

fn run(args: &[&str]) -> Result<String, String> {
    let out = command()?.args(args).output().map_err(|e| format!("Couldn't run Tailscale: {e}"))?;
    if !out.status.success() && out.stdout.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() { "Tailscale didn't answer.".into() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn status() -> Result<Status, String> {
    tailscale::parse_status(&run(&["status", "--json"])?).map_err(|e| e.to_string())
}

pub fn ping(ip: &str) -> Result<Option<Ping>, String> {
    if !tailscale::valid_target(ip) {
        return Err("That isn't a device address.".into());
    }
    let out = command()?.args(["ping", "-c", "1", "--timeout", "3s", "--until-direct=false", ip]).output().map_err(|e| e.to_string())?;
    Ok(tailscale::parse_ping(&String::from_utf8_lossy(&out.stdout)))
}

pub fn history() -> HashMap<String, Vec<Sample>> {
    HISTORY.lock().unwrap().as_ref().map(|h| h.iter().map(|(k, v)| (k.clone(), v.iter().cloned().collect())).collect()).unwrap_or_default()
}

/// Sends files to a device's Taildrop inbox.
pub fn send(target: &str, paths: &[String]) -> Result<(), String> {
    if !tailscale::valid_target(target) {
        return Err("That isn't a device.".into());
    }
    if paths.is_empty() {
        return Ok(());
    }
    let dest = format!("{target}:");
    let mut args: Vec<&str> = vec!["file", "cp", "--"];
    args.extend(paths.iter().map(String::as_str));
    args.push(&dest);
    let out = command()?.args(&args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("").to_string();
        Err(if err.is_empty() { "Taildrop didn't send the files.".into() } else { err })
    }
}

fn record(id: &str, s: Sample) {
    let mut h = HISTORY.lock().unwrap();
    let list = h.get_or_insert_with(HashMap::new).entry(id.to_string()).or_default();
    let since = s.at - DAY;
    list.push_back(s);
    while list.front().is_some_and(|x| x.at < since) {
        list.pop_front();
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

pub fn start(app: &AppHandle) {
    if cli().is_none() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut seen = HashMap::new();
        let mut warned: HashMap<String, i64> = HashMap::new();
        let mut minute = 0u64;
        loop {
            if let Some(s) = status().ok().filter(|s| s.running) {
                let watch = app.state::<AppState>().config.lock().map(|c| c.watch).unwrap_or(true);
                let now = chrono::Utc::now().timestamp();
                for (name, online) in tailscale::changes(&mut seen, &s.peers) {
                    if watch {
                        notify(&app, &name, if online { "Back on the tailnet." } else { "Went offline." });
                    }
                }
                for (name, days) in tailscale::expiring(&s.peers, now) {
                    let today = now / DAY;
                    if watch && warned.get(&name) != Some(&today) {
                        warned.insert(name.clone(), today);
                        let body = if days == 0 { "Its Tailscale key has expired. Sign in on it again.".to_string() } else { format!("Its Tailscale key expires in {days} day{}.", if days == 1 { "" } else { "s" }) };
                        notify(&app, &name, &body);
                    }
                }
                if minute % 5 == 0 {
                    let pings: Vec<_> = s
                        .peers
                        .iter()
                        .filter(|p| p.online && !p.this_mac)
                        .map(|p| {
                            let (id, ip) = (p.id.clone(), p.ip.clone());
                            std::thread::spawn(move || (id, ping(&ip).ok().flatten()))
                        })
                        .collect();
                    for h in pings {
                        if let Ok((id, p)) = h.join() {
                            record(&id, Sample { at: now, ms: p.as_ref().map(|p| p.ms), via: p.map(|p| p.via).unwrap_or_default() });
                        }
                    }
                }
            }
            minute += 1;
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

/// Sends this Mac's traffic through an exit node, by its Tailscale address, or stops it.
pub fn set_exit_node(exit: Option<&str>) -> Result<(), String> {
    let args = tailscale::exit_node_args(exit).map_err(|e| e.to_string())?;
    let out = command()?.args(&args).output().map_err(|e| format!("Couldn't run Tailscale: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("").to_string();
        Err(if err.is_empty() { "Tailscale didn't change the exit node.".into() } else { err })
    }
}
