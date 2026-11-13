//! The commands the interface calls. Each one locks what it needs, calls the core and turns errors into text.
//! Commands that touch the vault are `async`, so Tauri runs them off the main thread: a slow or stalled
//! SMB share must not freeze the window.

use crate::AppState;
use binder_core::config::{Config, PrinterConfig};
use binder_core::printer::PrinterCommand;
use crate::printer::{self, PrinterView};
use binder_core::model::*;
use binder_core::notes::{self, DoneLine};
use binder_core::remote::SshRemote;
use binder_core::repo::RepoStatus;
use binder_core::server::ServerStatus;
use binder_core::agent::{self, Access, AgentRun};
use binder_core::{Db, Error, devices, repo, server};
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

fn server_target(state: &State<AppState>) -> Res<String> {
    state.config.lock().unwrap().server.clone().filter(|s| !s.trim().is_empty()).ok_or_else(|| "No server set.".to_string())
}

/// One SSH call; the window keeps working while it runs.
#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Res<ServerStatus> {
    let target = server_target(&state)?;
    server::fetch(&SshRemote { target }, now()).map_err(text)
}

#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> Res<Vec<Device>> {
    state.db.lock().unwrap().list_devices().map_err(text)
}

#[tauri::command]
pub async fn create_device(state: State<'_, AppState>, device: NewDevice) -> Res<Device> {
    state.db.lock().unwrap().create_device(device).map_err(text)
}

#[tauri::command]
pub async fn update_device(state: State<'_, AppState>, id: i64, patch: DevicePatch) -> Res<Device> {
    state.db.lock().unwrap().update_device(id, patch).map_err(text)
}

#[tauri::command]
pub async fn delete_device(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.db.lock().unwrap().delete_device(id).map_err(text)
}

/// Probes every device at once, two seconds each at most.
#[tauri::command]
pub async fn probe_devices(state: State<'_, AppState>) -> Res<Vec<(i64, bool)>> {
    let list = state.db.lock().unwrap().list_devices().map_err(text)?;
    let handles: Vec<_> = list
        .into_iter()
        .map(|d| std::thread::spawn(move || (d.id, devices::probe(&d.address, d.port, std::time::Duration::from_secs(2)))))
        .collect();
    Ok(handles.into_iter().filter_map(|h| h.join().ok()).collect())
}

fn expand_home(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => std::path::PathBuf::from(home).join(rest),
        _ => std::path::PathBuf::from(path),
    }
}

/// None when the project has no repo, or its folder isn't a git repository any more.
#[tauri::command]
pub async fn repo_status(state: State<'_, AppState>, project_id: i64) -> Res<Option<RepoStatus>> {
    let p = state.db.lock().unwrap().get_project(project_id).map_err(text)?;
    Ok(p.repo.and_then(|r| repo::repo_status(&expand_home(&r)).ok()))
}

fn remote(state: &State<AppState>) -> Res<SshRemote> {
    Ok(SshRemote { target: server_target(state)? })
}

// The agent commands hold the database lock through one SSH call. Other commands wait for it, the window doesn't.
#[tauri::command]
pub async fn start_run(state: State<'_, AppState>, task_id: i64, dir: String, access: Access) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::start_run(&mut state.db.lock().unwrap(), &r, task_id, &dir, access, now()).map_err(text)
}

#[tauri::command]
pub async fn poll_run(state: State<'_, AppState>, run: String) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::poll_run(&mut state.db.lock().unwrap(), &r, &run, now()).map_err(text)
}

#[tauri::command]
pub async fn stop_run(state: State<'_, AppState>, run: String) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::stop_run(&mut state.db.lock().unwrap(), &r, &run, now()).map_err(text)
}

#[tauri::command]
pub async fn runs_for(state: State<'_, AppState>, task_id: i64) -> Res<Vec<AgentRun>> {
    agent::list_runs(&state.db.lock().unwrap(), task_id).map_err(text)
}

#[tauri::command]
pub async fn printers(state: State<'_, AppState>) -> Res<Vec<PrinterView>> {
    let list = state.config.lock().unwrap().printers.clone();
    Ok(list
        .into_iter()
        .map(|c| state.printers.view(&c.serial).unwrap_or(PrinterView { config: c, state: Default::default(), online: false, error: None }))
        .collect())
}

/// The access code goes straight to the Keychain. Only the name, address and serial reach the config.
#[tauri::command]
pub async fn add_printer(app: tauri::AppHandle, state: State<'_, AppState>, name: String, address: String, serial: String, access_code: String) -> Res<()> {
    let (name, address, serial, code) = (name.trim(), address.trim(), serial.trim(), access_code.trim());
    if name.is_empty() || address.is_empty() || serial.is_empty() || code.is_empty() {
        return Err("A printer needs a name, an address, a serial and an access code.".into());
    }
    if state.config.lock().unwrap().printers.iter().any(|p| p.serial == serial) {
        return Err("That printer is already added.".into());
    }
    printer::keychain(serial)
        .and_then(|e| e.set_password(code))
        .map_err(|_| "Couldn't save the access code in the Keychain.".to_string())?;
    let config = PrinterConfig { name: name.into(), address: address.into(), serial: serial.into(), cert_sha256: None };
    {
        let mut c = state.config.lock().unwrap();
        c.printers.push(config.clone());
        c.save(&state.config_path).map_err(text)?;
    }
    printer::start(&app, config);
    Ok(())
}

#[tauri::command]
pub async fn remove_printer(state: State<'_, AppState>, serial: String) -> Res<()> {
    state.printers.stop(&serial);
    if let Ok(entry) = printer::keychain(&serial) {
        let _ = entry.delete_credential();
    }
    let mut c = state.config.lock().unwrap();
    c.printers.retain(|p| p.serial != serial);
    c.save(&state.config_path).map_err(text)
}

#[tauri::command]
pub async fn printer_command(app: tauri::AppHandle, serial: String, kind: PrinterCommand) -> Res<()> {
    printer::send(&app, &serial, kind).await
}
