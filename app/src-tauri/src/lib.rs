mod apps;
mod commands;
mod printer;
mod terminal;
mod transfers;

use binder_core::{Db, config::Config};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub db: Mutex<Db>,
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    pub printers: printer::Printers,
    pub transfers: std::sync::Arc<transfers::Transfers>,
    /// Whether the Cloud fast path is on, checked once per run.
    pub fast: Mutex<Option<bool>>,
    /// The Quick Look window, while it's open.
    pub quicklook: Mutex<Option<std::process::Child>>,
    /// The Agents screen's two terminals.
    pub terminals: terminal::Terminals,
}

#[derive(Clone, Copy, serde::Serialize)]
struct DropPoint {
    x: f64,
    y: f64,
}

#[derive(Clone, serde::Serialize)]
struct Dropped {
    paths: Vec<PathBuf>,
    x: f64,
    y: f64,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let config_path = dir.join("config.toml");
            let config = Config::load(&config_path)?;
            let db = Db::open(&dir.join("binder.db"))?;
            let printers = config.printers.clone();
            app.manage(AppState { db: Mutex::new(db), config: Mutex::new(config), config_path, printers: Default::default(), transfers: Default::default(), fast: Default::default(), quicklook: Default::default(), terminals: Default::default() });
            for p in printers {
                printer::start(app.handle(), p);
            }
            // Done lines that waited for the vault get written now, off the main thread in case the share is slow.
            let data = dir.clone();
            std::thread::spawn(move || {
                let (Ok(config), Ok(mut db)) = (Config::load(&data.join("config.toml")), Db::open(&data.join("binder.db"))) else { return };
                let _ = binder_core::session::flush_pending(&mut db, &config.vault);
                if config.claude_hooks {
                    commands::repair_hooks();
                }
            });
            // Repos come in from GitHub at start; the interface reloads projects when this lands.
            let handle = app.handle().clone();
            let data = dir.clone();
            std::thread::spawn(move || commands::sync_and_tell(&handle, &data));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Files dragged over or dropped on the window go to the interface with the pointer in CSS pixels, so it
            // can tell which folder is under it: Cloud paths are a move, others an upload.
            let scale = window.scale_factor().unwrap_or(1.0);
            let at = |p: &tauri::PhysicalPosition<f64>| DropPoint { x: p.x / scale, y: p.y / scale };
            match event {
                tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, position }) => {
                    let p = at(position);
                    let _ = window.emit("cloud://drop", Dropped { paths: paths.clone(), x: p.x, y: p.y });
                }
                tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Over { position }) => {
                    let _ = window.emit("cloud://drag-over", Some(at(position)));
                }
                tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Leave) => {
                    let _ = window.emit("cloud://drag-over", None::<DropPoint>);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::delete_task,
            commands::list_projects,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::start_project,
            commands::open_note,
            commands::log_done,
            commands::read_done,
            commands::done_today,
            commands::last_done,
            commands::get_config,
            commands::set_config,
            commands::vault_connected,
            commands::server_status,
            commands::list_devices,
            commands::create_device,
            commands::update_device,
            commands::delete_device,
            commands::probe_devices,
            commands::repo_status,
            commands::start_run,
            commands::poll_run,
            commands::stop_run,
            commands::runs_for,
            commands::printers,
            commands::add_printer,
            commands::remove_printer,
            commands::printer_command,
            commands::claude_hooks,
            commands::set_claude_hooks,
            commands::github_sync,
            commands::open_repo_page,
            commands::hidden_repos,
            commands::unhide_repo,
            commands::cloud_status,
            commands::cloud_list,
            commands::cloud_mkdir,
            commands::cloud_rename,
            commands::cloud_move,
            commands::cloud_trash,
            commands::cloud_trash_list,
            commands::cloud_restore,
            commands::cloud_empty_trash,
            commands::cloud_open,
            commands::cloud_connect,
            commands::cloud_upload,
            commands::cloud_download,
            commands::cloud_cancel,
            commands::cloud_copy,
            commands::cloud_duplicate,
            commands::cloud_compress,
            commands::cloud_extract,
            commands::cloud_info,
            commands::cloud_size,
            commands::cloud_space,
            commands::cloud_fast_path,
            commands::cloud_quicklook,
            commands::cloud_apps_for,
            commands::cloud_open_with,
            commands::term_open,
            commands::term_write,
            commands::term_resize,
            commands::term_reconnect,
        ])
        .build(tauri::generate_context!())
        .expect("Binder could not start")
        .run(|app, event| {
            // Quitting ends the local ssh processes; the tmux sessions on the server stay.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.terminals.close_all();
                }
            }
        });
}
