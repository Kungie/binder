//! The commands the interface calls. Each one locks what it needs, calls the core and turns errors into text.
//! Commands that touch the vault are `async`, so Tauri runs them off the main thread: a slow or stalled
//! SMB share must not freeze the window.

use crate::AppState;
use binder_core::config::Config;
use binder_core::model::*;
use binder_core::notes::{self, DoneLine};
use binder_core::{Db, Error};
use std::path::Path;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

type Res<T> = Result<T, String>;

fn text(e: Error) -> String {
    e.to_string()
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn vault(state: &State<AppState>) -> std::path::PathBuf {
    state.config.lock().unwrap().vault.clone()
}

/// The project with its note, made from the template first when it has none (or it went missing).
fn note_for(db: &mut Db, vault: &Path, id: i64) -> Result<Project, Error> {
    let p = db.get_project(id)?;
    if p.note.as_deref().is_some_and(|n| Path::new(n).exists()) {
        return Ok(p);
    }
    let (path, _) = notes::ensure_note(vault, &p, &today())?;
    db.set_project_note(id, &path.to_string_lossy())?;
    db.get_project(id)
}

#[tauri::command]
pub fn list_tasks(state: State<AppState>, view: TaskView, include_closed: bool) -> Res<Vec<Task>> {
    state.db.lock().unwrap().list_tasks(view, include_closed).map_err(text)
}

#[tauri::command]
pub fn create_task(state: State<AppState>, task: NewTask) -> Res<Task> {
    state.db.lock().unwrap().create_task(task, now()).map_err(text)
}

#[tauri::command]
pub fn update_task(state: State<AppState>, id: i64, patch: TaskPatch) -> Res<Task> {
    state.db.lock().unwrap().update_task(id, patch, now()).map_err(text)
}

#[tauri::command]
pub fn delete_task(state: State<AppState>, id: i64) -> Res<()> {
    state.db.lock().unwrap().delete_task(id).map_err(text)
}

#[tauri::command]
pub fn list_projects(state: State<AppState>) -> Res<Vec<Project>> {
    state.db.lock().unwrap().list_projects().map_err(text)
}

#[tauri::command]
pub fn create_project(state: State<AppState>, project: NewProject) -> Res<Project> {
    state.db.lock().unwrap().create_project(project, now()).map_err(text)
}

#[tauri::command]
pub fn update_project(state: State<AppState>, id: i64, patch: ProjectPatch) -> Res<Project> {
    state.db.lock().unwrap().update_project(id, patch).map_err(text)
}

#[tauri::command]
pub fn delete_project(state: State<AppState>, id: i64) -> Res<()> {
    state.db.lock().unwrap().delete_project(id).map_err(text)
}

/// Makes the note if needed and opens it in Obsidian.
fn start(app: &tauri::AppHandle, state: &State<AppState>, id: i64) -> Res<Project> {
    let vault = vault(state);
    let p = note_for(&mut state.db.lock().unwrap(), &vault, id).map_err(text)?;
    let note = p.note.clone().expect("note_for sets the note");
    app.opener().open_url(notes::obsidian_url(Path::new(&note)), None::<&str>).map_err(|e| e.to_string())?;
    Ok(p)
}

#[tauri::command]
pub async fn start_project(app: tauri::AppHandle, state: State<'_, AppState>, id: i64) -> Res<Project> {
    start(&app, &state, id)
}

#[tauri::command]
pub async fn open_note(app: tauri::AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    start(&app, &state, id).map(|_| ())
}

#[tauri::command]
pub async fn log_done(state: State<'_, AppState>, project_id: i64, text: String) -> Res<bool> {
    let vault = vault(&state);
    let p = note_for(&mut state.db.lock().unwrap(), &vault, project_id).map_err(self::text)?;
    let note = p.note.expect("note_for sets the note");
    notes::append_done(Path::new(&note), &today(), &text).map_err(self::text)
}

#[tauri::command]
pub async fn read_done(state: State<'_, AppState>, project_id: i64) -> Res<Vec<DoneLine>> {
    let p = state.db.lock().unwrap().get_project(project_id).map_err(text)?;
    match p.note {
        Some(note) => notes::read_done(Path::new(&note)).map_err(text),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub async fn done_today(state: State<'_, AppState>) -> Res<Vec<(String, DoneLine)>> {
    let projects = state.db.lock().unwrap().list_projects().map_err(text)?;
    Ok(notes::done_on(&projects, &today()))
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn set_config(state: State<AppState>, config: Config) -> Res<()> {
    config.save(&state.config_path).map_err(text)?;
    *state.config.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
pub async fn vault_connected(state: State<'_, AppState>) -> Res<bool> {
    Ok(vault(&state).is_dir())
}
