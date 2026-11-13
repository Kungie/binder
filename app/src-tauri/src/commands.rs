//! The commands the interface calls. Each one locks what it needs, calls the core and turns errors into text.
//! Every command is `async`, so Tauri runs it off the main thread: a slow SMB share, SSH call or database lock
//! must not freeze the window.

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
use binder_core::cloud::{self, Conflict, Space};
use binder_core::github::{self, GhCli, SyncReport};
use binder_core::{Db, Error, devices, hooks, repo, server};
use tauri::Emitter;
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
pub async fn list_tasks(state: State<'_, AppState>, view: TaskView, include_closed: bool) -> Res<Vec<Task>> {
    state.db.lock().unwrap().list_tasks(view, include_closed).map_err(text)
}

#[tauri::command]
pub async fn create_task(state: State<'_, AppState>, task: NewTask) -> Res<Task> {
    state.db.lock().unwrap().create_task(task, now()).map_err(text)
}

#[tauri::command]
pub async fn update_task(state: State<'_, AppState>, id: i64, patch: TaskPatch) -> Res<Task> {
    state.db.lock().unwrap().update_task(id, patch, now()).map_err(text)
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.db.lock().unwrap().delete_task(id).map_err(text)
}

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Res<Vec<Project>> {
    state.db.lock().unwrap().list_projects().map_err(text)
}

#[tauri::command]
pub async fn create_project(state: State<'_, AppState>, project: NewProject) -> Res<Project> {
    state.db.lock().unwrap().create_project(project, now()).map_err(text)
}

#[tauri::command]
pub async fn update_project(state: State<'_, AppState>, id: i64, patch: ProjectPatch) -> Res<Project> {
    state.db.lock().unwrap().update_project(id, patch).map_err(text)
}

#[tauri::command]
pub async fn delete_project(state: State<'_, AppState>, id: i64) -> Res<()> {
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
pub async fn get_config(state: State<'_, AppState>) -> Res<Config> {
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, config: Config) -> Res<()> {
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
    server::fetch(&SshRemote::new(target), now()).map_err(text)
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
    Ok(SshRemote::new(server_target(state)?))
}

// The agent functions lock the database only around reads and writes, never during the SSH call.
#[tauri::command]
pub async fn start_run(state: State<'_, AppState>, task_id: i64, dir: String, access: Access) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::start_run(&state.db, &r, task_id, &dir, access, now()).map_err(text)
}

#[tauri::command]
pub async fn poll_run(state: State<'_, AppState>, run: String) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::poll_run(&state.db, &r, &run, now()).map_err(text)
}

#[tauri::command]
pub async fn stop_run(state: State<'_, AppState>, run: String) -> Res<AgentRun> {
    let r = remote(&state)?;
    agent::stop_run(&state.db, &r, &run, now()).map_err(text)
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

fn claude_settings() -> Res<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".claude/settings.json")).ok_or_else(|| "HOME isn't set.".into())
}

#[tauri::command]
pub async fn claude_hooks() -> Res<bool> {
    Ok(hooks::installed(&claude_settings()?))
}

/// binder-cli sits beside the app's own executable, in the bundle and in a dev build alike.
#[tauri::command]
pub async fn set_claude_hooks(state: State<'_, AppState>, on: bool) -> Res<bool> {
    let settings = claude_settings()?;
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let cli = exe.with_file_name("binder-cli");
        if !cli.is_file() {
            return Err(format!("binder-cli is missing at {}", cli.display()));
        }
        hooks::install(&settings, &cli).map_err(text)?;
    } else {
        hooks::uninstall(&settings).map_err(text)?;
    }
    let mut c = state.config.lock().unwrap();
    c.claude_hooks = on;
    c.save(&state.config_path).map_err(text)?;
    Ok(hooks::installed(&settings))
}

/// Brings GitHub repos in on its own database connection, so the app's lock isn't held while `gh` runs.
pub fn sync_github(data_dir: &Path) -> Result<SyncReport, Error> {
    let gh = GhCli::find().ok_or_else(|| Error::Invalid("Install gh and sign in with gh auth login to bring in your GitHub repos.".into()))?;
    let mut db = Db::open(&data_dir.join("binder.db"))?;
    github::sync(&mut db, &gh, now())
}

/// Opens a project's GitHub page. Only github.com addresses are opened.
#[tauri::command]
pub async fn open_repo_page(app: tauri::AppHandle, state: State<'_, AppState>, project_id: i64) -> Res<()> {
    let url = state.db.lock().unwrap().get_project(project_id).map_err(text)?.url;
    let url = url.filter(|u| u.starts_with("https://github.com/")).ok_or_else(|| "This project isn't on GitHub.".to_string())?;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// What `projects://changed` carries: when a sync ran and, if it failed, why.
#[derive(Clone, serde::Serialize)]
pub struct GithubSync {
    pub at: i64,
    pub error: Option<String>,
}

/// Syncs and tells the interface, which reloads its projects. Used at start.
pub fn sync_and_tell(app: &tauri::AppHandle, data_dir: &Path) {
    let error = sync_github(data_dir).err().map(text);
    let _ = app.emit("projects://changed", GithubSync { at: now(), error });
}

#[tauri::command]
pub async fn github_sync(state: State<'_, AppState>) -> Res<SyncReport> {
    let dir = state.config_path.parent().expect("config sits in the data folder").to_path_buf();
    sync_github(&dir).map_err(text)
}

#[derive(serde::Serialize)]
pub struct CloudStatus {
    pub mounted: bool,
    pub path: String,
    pub space: Option<Space>,
}

/// A share is mounted when its folder sits on another device than its parent. An empty `/Volumes/cloud` left on the
/// Mac's own disk is not Cloud, and nothing gets written into it.
fn mounted(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(me), Some(Ok(up))) = (std::fs::metadata(path), path.parent().map(std::fs::metadata)) else { return false };
    me.is_dir() && me.dev() != up.dev()
}

fn cloud_root(state: &State<AppState>) -> Res<std::path::PathBuf> {
    let root = state.config.lock().unwrap().cloud.clone();
    if mounted(&root) { Ok(root) } else { Err("Cloud not connected.".into()) }
}

#[tauri::command]
pub async fn cloud_status(state: State<'_, AppState>) -> Res<CloudStatus> {
    let root = state.config.lock().unwrap().cloud.clone();
    let on = mounted(&root);
    Ok(CloudStatus { mounted: on, path: root.to_string_lossy().into_owned(), space: if on { cloud::space(&root).ok() } else { None } })
}

#[tauri::command]
pub async fn cloud_list(state: State<'_, AppState>, rel: String, show_hidden: bool) -> Res<Vec<cloud::Entry>> {
    cloud::list(&cloud_root(&state)?, &rel, show_hidden).map_err(text)
}

#[tauri::command]
pub async fn cloud_mkdir(state: State<'_, AppState>, rel_dir: String, name: String) -> Res<()> {
    cloud::mkdir(&cloud_root(&state)?, &rel_dir, &name).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_rename(state: State<'_, AppState>, rel: String, new_name: String) -> Res<()> {
    cloud::rename(&cloud_root(&state)?, &rel, &new_name).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_move(state: State<'_, AppState>, rel: String, dest_dir_rel: String) -> Res<()> {
    cloud::move_to(&cloud_root(&state)?, &rel, &dest_dir_rel).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_trash(state: State<'_, AppState>, rel: String) -> Res<()> {
    cloud::trash(&cloud_root(&state)?, &rel).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_trash_list(state: State<'_, AppState>) -> Res<Vec<cloud::TrashEntry>> {
    cloud::list_trash(&cloud_root(&state)?).map_err(text)
}

#[tauri::command]
pub async fn cloud_restore(state: State<'_, AppState>, id: String) -> Res<()> {
    cloud::restore(&cloud_root(&state)?, &id).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_empty_trash(state: State<'_, AppState>) -> Res<()> {
    cloud::empty_trash(&cloud_root(&state)?).map_err(text)
}

#[tauri::command]
pub async fn cloud_open(app: tauri::AppHandle, state: State<'_, AppState>, rel: String) -> Res<()> {
    let path = cloud::resolve(&cloud_root(&state)?, &rel).map_err(text)?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cloud_connect(app: tauri::AppHandle, state: State<'_, AppState>) -> Res<()> {
    let share = state.config.lock().unwrap().cloud_share.clone().filter(|s| !s.trim().is_empty());
    let share = share.ok_or_else(|| "Set the share address in Settings.".to_string())?;
    if !share.starts_with("smb://") {
        return Err("The share address starts with smb://.".into());
    }
    app.opener().open_url(share, None::<&str>).map_err(|e| e.to_string())
}

/// `trash_root` is Cloud's root for uploads, where Replace moves the old item to the trash; downloads to the Mac pass
/// `None`, which refuses Replace.
fn copies(
    app: &tauri::AppHandle,
    state: &State<AppState>,
    jobs: Vec<(std::path::PathBuf, std::path::PathBuf)>,
    conflict: Conflict,
    trash_root: Option<std::path::PathBuf>,
) -> Vec<u64> {
    jobs.into_iter()
        .map(|(src, dst_dir)| {
            let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let app = app.clone();
            let trash_root = trash_root.clone();
            state.transfers.spawn(name, move |e| { let _ = app.emit("cloud://transfer", e); }, move |cancel, progress| {
                cloud::copy_into(&src, &dst_dir, conflict, trash_root.as_deref(), cancel, progress).map(|_| ())
            })
        })
        .collect()
}

/// Copies files or folders from the Mac into a Cloud folder. Returns the transfer ids.
#[tauri::command]
pub async fn cloud_upload(app: tauri::AppHandle, state: State<'_, AppState>, paths: Vec<String>, dest_rel: String, conflict: Conflict) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let dest = cloud::resolve(&root, &dest_rel).map_err(text)?;
    if !dest.is_dir() {
        return Err("Upload into a folder.".into());
    }
    let jobs = paths.into_iter().map(|p| (std::path::PathBuf::from(p), dest.clone())).collect();
    Ok(copies(&app, &state, jobs, conflict, Some(root)))
}

/// Copies Cloud items into a folder on the Mac. Returns the transfer ids.
#[tauri::command]
pub async fn cloud_download(app: tauri::AppHandle, state: State<'_, AppState>, rels: Vec<String>, dest_dir: String, conflict: Conflict) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let dest = std::path::PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err("Save into a folder.".into());
    }
    let jobs = rels.iter().map(|r| cloud::resolve(&root, r).map(|src| (src, dest.clone()))).collect::<Result<Vec<_>, _>>().map_err(text)?;
    Ok(copies(&app, &state, jobs, conflict, None))
}

#[tauri::command]
pub async fn cloud_cancel(state: State<'_, AppState>, id: u64) -> Res<()> {
    state.transfers.cancel(id);
    Ok(())
}
