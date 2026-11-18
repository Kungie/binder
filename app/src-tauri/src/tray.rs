//! The menu bar: shown only while a task is in progress or a pomodoro runs. It lists the tasks in progress, holds a
//! plain pomodoro (its time left beside the icon), New task, Open Binder and Quit.

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

/// The tasks in progress, as the menu shows them: key and title.
fn in_progress(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let db = state.db.lock().unwrap();
    db.list_tasks(binder_core::model::TaskView::All, false)
        .map(|l| l.into_iter().filter(|t| t.status == binder_core::model::Status::InProgress).map(|t| format!("{}  {}", t.key, t.title)).collect())
        .unwrap_or_default()
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let p = app.state::<Timer>().0.lock().unwrap().clone();
    let item = |id: &str, text: &str, on: bool, key: Option<&str>| MenuItem::with_id(app, id, text, on, key);
    let m = Menu::new(app)?;
    let tasks = in_progress(app);
    if !tasks.is_empty() {
        m.append(&item("doing", "In progress", false, None)?)?;
        for (i, t) in tasks.iter().take(8).enumerate() {
            let short: String = if t.chars().count() > 48 { format!("{}…", t.chars().take(47).collect::<String>()) } else { t.clone() };
            m.append(&item(&format!("task-{i}"), &short, true, None)?)?;
        }
        m.append(&PredefinedMenuItem::separator(app)?)?;
    }
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

/// Shows the icon while a task is in progress or a pomodoro runs, hides it otherwise, and rebuilds the menu when the
/// tasks in progress change.
fn show_when_busy(app: &AppHandle) {
    static LAST: Mutex<Option<(bool, Vec<String>)>> = Mutex::new(None);
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let tasks = in_progress(app);
    let timing = app.state::<Timer>().0.lock().unwrap().phase().is_some();
    let visible = !tasks.is_empty() || timing;
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let now = Some((visible, tasks));
    if *last != now {
        let _ = tray.set_visible(visible);
        *last = now;
        drop(last);
        refresh(app);
    }
}

/// The time left beside the icon; the menu bar is only touched when the text changes.
fn title(app: &AppHandle) {
    static SHOWN: Mutex<Option<String>> = Mutex::new(None);
    let Some(tray) = app.tray_by_id(TRAY) else { return };
    let label = app.state::<Timer>().0.lock().unwrap().label(now());
    let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
    if *shown != label {
        let _ = tray.set_title(label.as_deref());
        *shown = label;
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// When the quick add window was last hidden; it is closed after 5 minutes unused, so its web view doesn't hold
/// memory all day.
static QUICK_HIDDEN: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// The quick add window, centred and focused, its field cleared; made on first use.
pub fn show_quick(app: &AppHandle) {
    *QUICK_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("quick://open", ());
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "quick", tauri::WebviewUrl::App("index.html#quick".into()))
        .title("New task")
        .inner_size(560.0, 64.0)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .focused(true)
        .background_color(tauri::window::Color(0x14, 0x14, 0x16, 0xff))
        .build();
}

pub fn quick_hidden() {
    *QUICK_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
}

/// Closes the quick add window once it has been hidden 5 minutes.
fn close_idle_quick(app: &AppHandle) {
    let idle = QUICK_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| t.elapsed() >= Duration::from_secs(300));
    if idle {
        if let Some(w) = app.get_webview_window("quick") {
            let _ = w.destroy();
        }
        *QUICK_HIDDEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
        _ if id.starts_with("task-") => show_main(app),
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
    show_when_busy(app);
    let a = app.clone();
    let mut tick = 0u32;
    std::thread::spawn(move || loop {
        tick += 1;
        if tick % 3 == 0 {
            show_when_busy(&a);
        }
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
        close_idle_quick(&a);
        // Idle, nothing moves: look again less often.
        let idle = a.state::<Timer>().0.lock().unwrap().phase().is_none();
        std::thread::sleep(Duration::from_secs(if idle { 2 } else { 1 }));
    });
    Ok(())
}
