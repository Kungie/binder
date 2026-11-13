mod commands;

use binder_core::{Db, config::Config};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Db>,
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
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
            app.manage(AppState { db: Mutex::new(db), config: Mutex::new(config), config_path });
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
        ])
        .run(tauri::generate_context!())
        .expect("Binder could not start");
}
