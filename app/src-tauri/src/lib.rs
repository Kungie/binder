mod commands;
mod printer;

use binder_core::{Db, config::Config};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Db>,
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    pub printers: printer::Printers,
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
            app.manage(AppState { db: Mutex::new(db), config: Mutex::new(config), config_path, printers: Default::default() });
            for p in printers {
                printer::start(app.handle(), p);
            }
            // Done lines that waited for the vault get written now, off the main thread in case the share is slow.
            let data = dir.clone();
            std::thread::spawn(move || {
                let (Ok(config), Ok(mut db)) = (Config::load(&data.join("config.toml")), Db::open(&data.join("binder.db"))) else { return };
                let _ = binder_core::session::flush_pending(&mut db, &config.vault);
            });
            Ok(())
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
        ])
        .run(tauri::generate_context!())
        .expect("Binder could not start");
}
