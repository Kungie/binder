//! Binder VPN: a menu bar app for friends. They sign in with a user name and a password; the sign-in service gives
//! a one-time Tailscale key, which adds a profile to their Tailscale app. Turning on sends their traffic through one
//! of the owner's exit nodes. Tailscale makes the tunnel; this app only tells it what to do.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use binder_core::tailscale;
use binder_core::vpn_client::{self, Region};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// What the app keeps between launches: who signed in, the Tailscale profile it made, the regions last seen.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Saved {
    #[serde(default)]
    server: String,
    user: String,
    hostname: String,
    profile: String,
    #[serde(default)]
    regions: Vec<Region>,
    #[serde(default)]
    last_exit: Option<String>,
}

#[derive(Default)]
struct State {
    saved: Option<Saved>,
    busy: Option<&'static str>,
    error: Option<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap();
    f(g.get_or_insert_with(State::default))
}

fn data_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("binder-vpn"));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn load(app: &AppHandle) -> Option<Saved> {
    serde_json::from_str(&std::fs::read_to_string(data_dir(app).join("state.json")).ok()?).ok()
}

fn store(app: &AppHandle, saved: Option<&Saved>) {
    let path = data_dir(app).join("state.json");
    match saved {
        Some(s) => {
            let _ = std::fs::write(&path, serde_json::to_string_pretty(s).unwrap_or_default());
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

// Tailscale's command line.

fn cli() -> Option<&'static str> {
    ["/Applications/Tailscale.app/Contents/MacOS/Tailscale", "/opt/homebrew/bin/tailscale", "/usr/local/bin/tailscale"].into_iter().find(|p| std::path::Path::new(p).exists())
}

/// Runs Tailscale with a deadline. The app's binary acts as the command line only with TAILSCALE_BE_CLI set.
fn ts(args: &[&str], deadline: Duration) -> Result<String, String> {
    let mut child = Command::new(cli().ok_or("Tailscale isn't installed. Get it from tailscale.com/download, open it once, then try again.")?)
        .args(args)
        .env("TAILSCALE_BE_CLI", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't run Tailscale: {e}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if started.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Tailscale didn't answer. Open the Tailscale app and allow its VPN configuration if it asks.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    if status.success() {
        Ok(out)
    } else {
        let err = err.trim().lines().last().unwrap_or("").to_string();
        Err(if err.is_empty() { "Tailscale refused.".into() } else { err })
    }
}

fn current_profile() -> Option<String> {
    vpn_client::current_profile(&ts(&["switch", "--list"], Duration::from_secs(10)).ok()?)
}

/// Makes the app's profile the one in use.
fn use_profile(saved: &Saved) -> Result<(), String> {
    if current_profile().as_deref() != Some(saved.profile.as_str()) {
        ts(&["switch", &saved.profile], Duration::from_secs(20))?;
    }
    Ok(())
}

// What the menu shows.

struct View {
    tailscale: bool,
    signed_in: Option<String>,
    on: Option<Region>,
    regions: Vec<Region>,
    chosen: Option<String>,
}

/// Reads Tailscale and keeps the saved regions fresh while the app's profile is in use.
fn read(app: &AppHandle) -> View {
    let saved = with(|s| s.saved.clone());
    let mut view = View { tailscale: cli().is_some(), signed_in: None, on: None, regions: vec![], chosen: None };
    let Some(mut saved) = saved else { return view };
    view.signed_in = Some(saved.user.clone());
    let ours = view.tailscale && current_profile().as_deref() == Some(saved.profile.as_str());
    if ours {
        if let Ok(status) = ts(&["status", "--json"], Duration::from_secs(10)).and_then(|o| tailscale::parse_status(&o).map_err(|e| e.to_string())) {
            if status.running {
                let regions = vpn_client::regions(&status);
                if !regions.is_empty() && regions != saved.regions {
                    saved.regions = regions;
                    store(app, Some(&saved));
                    with(|s| s.saved = Some(saved.clone()));
                }
                view.on = saved.regions.iter().find(|r| r.active).cloned();
            }
        }
    }
    view.chosen = view.on.as_ref().map(|r| r.ip.clone()).or(saved.last_exit.clone()).or_else(|| saved.regions.iter().find(|r| r.online).map(|r| r.ip.clone()));
    view.regions = saved.regions.iter().map(|r| Region { active: view.on.as_ref().is_some_and(|o| o.ip == r.ip), ..r.clone() }).collect();
    view
}

fn menu(app: &AppHandle, view: &View) -> tauri::Result<Menu<tauri::Wry>> {
    let (busy, error) = with(|s| (s.busy, s.error.clone()));
    let m = Menu::new(app)?;
    let line = |text: &str| MenuItem::with_id(app, format!("info:{text}"), text, false, None::<&str>);
    let state = match (busy, &view.on) {
        (Some(word), _) => word.to_string(),
        (None, Some(r)) => format!("On · {}", r.city),
        (None, None) if view.signed_in.is_some() => "Off".into(),
        _ => "Not signed in".into(),
    };
    m.append(&line(&format!("Binder VPN: {state}"))?)?;
    if let Some(e) = &error {
        m.append(&line(e)?)?;
    }
    m.append(&PredefinedMenuItem::separator(app)?)?;
    if !view.tailscale {
        m.append(&line("Tailscale isn't installed")?)?;
        m.append(&MenuItem::with_id(app, "get-tailscale", "Get Tailscale…", true, None::<&str>)?)?;
    } else if let Some(user) = &view.signed_in {
        let can = busy.is_none() && (view.on.is_some() || view.chosen.is_some());
        m.append(&MenuItem::with_id(app, "toggle", if view.on.is_some() { "Turn Off" } else { "Turn On" }, can, Some("CmdOrCtrl+T"))?)?;
        if !view.regions.is_empty() {
            let sub = Submenu::with_id(app, "regions", "Region", true)?;
            for r in &view.regions {
                let label = if r.online { format!("{} ({})", r.city, r.name) } else { format!("{} ({}, offline)", r.city, r.name) };
                let checked = view.chosen.as_deref() == Some(r.ip.as_str());
                sub.append(&CheckMenuItem::with_id(app, format!("region:{}", r.ip), label, r.online && busy.is_none(), checked, None::<&str>)?)?;
            }
            m.append(&sub)?;
        }
        m.append(&PredefinedMenuItem::separator(app)?)?;
        m.append(&line(&format!("Signed in as {user}"))?)?;
        m.append(&MenuItem::with_id(app, "signout", "Sign Out", busy.is_none(), None::<&str>)?)?;
    } else {
        m.append(&MenuItem::with_id(app, "signin", "Sign In…", true, None::<&str>)?)?;
    }
    m.append(&PredefinedMenuItem::separator(app)?)?;
    m.append(&MenuItem::with_id(app, "quit", "Quit Binder VPN", true, Some("CmdOrCtrl+Q"))?)?;
    Ok(m)
}

fn refresh(app: &AppHandle) {
    let view = read(app);
    let app2 = app.clone();
    let on = view.on.is_some();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app2.tray_by_id("main") {
            if let Ok(m) = menu(&app2, &view) {
                let _ = tray.set_menu(Some(m));
            }
            let _ = tray.set_icon(Some(icon(on)));
            let _ = tray.set_icon_as_template(true);
        }
    });
}

fn icon(on: bool) -> Image<'static> {
    let bytes: &'static [u8] = if on { include_bytes!("../icons/tray-on.png") } else { include_bytes!("../icons/tray-off.png") };
    Image::from_bytes(bytes).expect("tray icon")
}

/// Runs a slow action off the main thread, with a word for the menu while it runs.
fn act(app: &AppHandle, word: &'static str, work: impl FnOnce(&AppHandle) -> Result<(), String> + Send + 'static) {
    with(|s| {
        s.busy = Some(word);
        s.error = None;
    });
    refresh(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let result = work(&app);
        with(|s| {
            s.busy = None;
            s.error = result.err();
        });
        refresh(&app);
    });
}

fn turn_on(app: &AppHandle, exit: String) -> Result<(), String> {
    let mut saved = with(|s| s.saved.clone()).ok_or("Sign in first.")?;
    use_profile(&saved)?;
    let args = vpn_client::up_args(&saved.hostname, &exit);
    ts(&args.iter().map(String::as_str).collect::<Vec<_>>(), Duration::from_secs(30))?;
    saved.last_exit = Some(exit);
    store(app, Some(&saved));
    with(|s| s.saved = Some(saved));
    Ok(())
}

fn turn_off() -> Result<(), String> {
    ts(&["set", "--exit-node="], Duration::from_secs(20)).map(drop)
}

fn sign_out(app: &AppHandle) -> Result<(), String> {
    if let Some(saved) = with(|s| s.saved.clone()) {
        if cli().is_some() && use_profile(&saved).is_ok() {
            let _ = ts(&["logout"], Duration::from_secs(20));
        }
    }
    store(app, None);
    with(|s| s.saved = None);
    Ok(())
}

/// Asks the service for a key with the user name and password, then signs Tailscale in with it as a new profile.
#[tauri::command]
async fn sign_in(app: AppHandle, server: String, user: String, password: String) -> Result<(), String> {
    if cli().is_none() {
        return Err("Tailscale isn't installed. Get it from tailscale.com/download, open it once, then try again.".into());
    }
    let url = vpn_client::login_url(&server).map_err(|_| "The server is the https address you were given.".to_string())?;
    let server = server.trim().trim_end_matches('/').to_string();
    let user = user.trim().to_lowercase();
    if user.is_empty() || password.is_empty() {
        return Err("Enter your user name and password.".into());
    }
    let _ = std::fs::write(data_dir(&app).join("server"), &server);
    // The password goes to curl on stdin, never in its arguments.
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "--max-time", "25", "--proto", "=https", "-H", "Content-Type: application/json", "--data-binary", "@-", "--max-filesize", "65536", "-w", "\n%{http_code}", "--", &url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Couldn't reach Binder VPN: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(vpn_client::login_body(&user, &password).as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("Couldn't reach Binder VPN. Check the internet connection.".into());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", "0"));
    let login = vpn_client::parse_login(body, code.trim().parse().unwrap_or(0))?;

    // The key lands in a file only this user can read, for as long as Tailscale needs it.
    let key_path = data_dir(&app).join("key");
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&key_path).map_err(|e| e.to_string())?;
        f.write_all(login.key.as_bytes()).map_err(|e| e.to_string())?;
    }
    let result = ts(
        &["login", &format!("--auth-key=file:{}", key_path.display()), &format!("--hostname={}", login.hostname)],
        Duration::from_secs(60),
    );
    let _ = std::fs::remove_file(&key_path);
    result?;
    let profile = current_profile().ok_or("Tailscale signed in, but its profile couldn't be read.")?;
    let saved = Saved { server, user, hostname: login.hostname, profile, regions: vec![], last_exit: None };
    store(&app, Some(&saved));
    with(|s| {
        s.saved = Some(saved);
        s.error = None;
    });
    if let Some(w) = app.get_webview_window("signin") {
        let _ = w.close();
    }
    let app2 = app.clone();
    std::thread::spawn(move || refresh(&app2));
    Ok(())
}

/// The server last signed in to, for the sign-in window.
#[tauri::command]
fn last_server(app: AppHandle) -> String {
    std::fs::read_to_string(data_dir(&app).join("server")).unwrap_or_default().trim().to_string()
}

fn show_sign_in(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("signin") {
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "signin", WebviewUrl::App("index.html".into()))
        .title("Binder VPN")
        .inner_size(340.0, 480.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .focused(true)
        .build();
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![sign_in, last_server])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let handle = app.handle().clone();
            let saved = load(&handle);
            let first = saved.is_none();
            with(|s| s.saved = saved);
            TrayIconBuilder::with_id("main")
                .icon(icon(false))
                .icon_as_template(true)
                .tooltip("Binder VPN")
                .menu(&Menu::new(&handle)?)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref().to_string();
                    match id.as_str() {
                        "quit" => app.exit(0),
                        "signin" => show_sign_in(app),
                        "get-tailscale" => {
                            let _ = Command::new("/usr/bin/open").arg("https://tailscale.com/download/mac").spawn();
                        }
                        "signout" => act(app, "Signing out", sign_out),
                        "toggle" => {
                            let view = read(app);
                            if view.on.is_some() {
                                act(app, "Turning off", |_| turn_off());
                            } else if let Some(exit) = view.chosen {
                                act(app, "Connecting", move |app| turn_on(app, exit));
                            }
                        }
                        _ => {
                            if let Some(ip) = id.strip_prefix("region:") {
                                let ip = ip.to_string();
                                let on = read(app).on.is_some();
                                if on {
                                    act(app, "Connecting", move |app| turn_on(app, ip));
                                } else {
                                    // Off stays off: the region is the one Turn On uses next.
                                    if let Some(mut saved) = with(|s| s.saved.clone()) {
                                        saved.last_exit = Some(ip);
                                        store(app, Some(&saved));
                                        with(|s| s.saved = Some(saved));
                                    }
                                    refresh(app);
                                }
                            }
                        }
                    }
                })
                .build(app)?;
            std::thread::spawn(move || loop {
                refresh(&handle);
                std::thread::sleep(Duration::from_secs(10));
            });
            if first {
                show_sign_in(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Binder VPN couldn't start")
        .run(|_app, event| {
            // Closing the sign-in window leaves the menu bar icon running.
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
