//! The menu bar: a plain pomodoro (its time left beside the icon), New task, Open Binder and Quit.

use crate::AppState;
use binder_core::pomodoro::{Phase, Pomodoro};
use std::sync::Mutex;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;

const TRAY: &str = "binder";

#[derive(Default)]
pub struct Timer(pub Mutex<Pomodoro>);

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Focus sessions finished since local midnight.
fn today(app: &AppHandle) -> i64 {
    use chrono::TimeZone;
    let midnight = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|m| chrono::Local.from_local_datetime(&m).single()).map(|m| m.timestamp()).unwrap_or(0);
    let state = app.state::<AppState>();
    let db = state.db.lock().unwrap();
    binder_core::pomodoro::count_since(&db, midnight).unwrap_or(0)
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let p = app.state::<Timer>().0.lock().unwrap().clone();
    let item = |id: &str, text: &str, on: bool, key: Option<&str>| MenuItem::with_id(app, id, text, on, key);
    let m = Menu::new(app)?;
    match p.phase() {
        None => m.append(&item("focus", "Start focus", true, None)?)?,
        Some(phase) => {
            let name = match phase {
                Phase::Focus => "Focus",
                Phase::ShortBreak => "Short break",
                Phase::LongBreak => "Long break",
            };
            let head = if p.paused() { format!("{name}, paused") } else { name.to_string() };
            m.append(&item("phase", &head, false, None)?)?;
            m.append(&item(if p.paused() { "resume" } else { "pause" }, if p.paused() { "Resume" } else { "Pause" }, true, None)?)?;
            m.append(&item("stop", if phase == Phase::Focus { "Stop" } else { "Skip break" }, true, None)?)?;
        }
    }
    let n = today(app);
    m.append(&item("count", &format!("{n} focus session{} today", if n == 1 { "" } else { "s" }), false, None)?)?;
    m.append(&PredefinedMenuItem::separator(app)?)?;
    m.append(&item("new_task", "New task…", true, Some("Ctrl+Alt+N"))?)?;
    m.append(&item("open", "Open Binder", true, None)?)?;
    m.append(&PredefinedMenuItem::separator(app)?)?;
    m.append(&item("quit", "Quit Binder", true, None)?)?;
    Ok(m)
}

fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    if let Ok(m) = menu(app) {
        let _ = tray.set_menu(Some(m));
    }
    title(app);
}

fn title(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let label = app.state::<Timer>().0.lock().unwrap().label(now());
    let _ = tray.set_title(label.as_deref());
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// The quick add window, centred and focused, its field cleared.
pub fn show_quick(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("quick://open", ());
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    {
        let timer = app.state::<Timer>();
        let mut p = timer.0.lock().unwrap();
        match id {
            "focus" => p.focus(now()),
            "pause" => p.pause(now()),
            "resume" => p.resume(now()),
            "stop" => p.stop(),
            _ => {}
        }
    }
    match id {
        "new_task" => show_quick(app),
        "open" => show_main(app),
        "quit" => app.exit(0),
        _ => {}
    }
    refresh(app);
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).sound("default").show();
}

pub fn start(app: &AppHandle) -> tauri::Result<()> {
    app.manage(Timer::default());
    TrayIconBuilder::with_id(TRAY)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| on_menu(app, e.id().as_ref()))
        .build(app)?;
    let a = app.clone();
    std::thread::spawn(move || loop {
        let ended = a.state::<Timer>().0.lock().unwrap().tick(now());
        if let Some((phase, next)) = ended {
            if phase == Phase::Focus {
                let state = a.state::<AppState>();
                let _ = binder_core::pomodoro::record(&mut state.db.lock().unwrap(), now(), 25);
                let body = if next == Some(Phase::LongBreak) { "Take 15 minutes." } else { "Take 5 minutes." };
                notify(&a, "Focus done", body);
            } else {
                notify(&a, "Break over", "Start the next focus from the menu bar.");
            }
            refresh(&a);
        } else {
            title(&a);
        }
        std::thread::sleep(Duration::from_secs(1));
    });
    Ok(())
}
