mod commands;
mod printer;
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
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let config_path = dir.join("config.toml");
            let config = Config::load(&config_path)?;
            let db = Db::open(&dir.join("binder.db"))?;
            let printers = config.printers.clone();
            app.manage(AppState { db: Mutex::new(db), config: Mutex::new(config), config_path, printers: Default::default(), transfers: Default::default() });
            for p in printers {
                printer::start(app.handle(), p);
            }
            // Done lines that waited for the vault get written now, off the main thread in case the share is slow.
            let data = dir.clone();
            std::thread::spawn(move || {
                let (Ok(config), Ok(mut db)) = (Config::load(&data.join("config.toml")), Db::open(&data.join("binder.db"))) else { return };
                let _ = binder_core::session::flush_pending(&mut db, &config.vault);
            });
            // Repos come in from GitHub at start; the interface reloads projects when this lands.
            let handle = app.handle().clone();
            let data = dir.clone();
            std::thread::spawn(move || commands::sync_and_tell(&handle, &data));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Files dropped from Finder go to the interface, which decides whether Cloud takes them.
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let _ = window.emit("cloud://drop", paths);
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
        ])
        .run(tauri::generate_context!())
        .expect("Binder could not start");
}
