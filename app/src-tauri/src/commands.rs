//! The commands the interface calls. Each one locks what it needs, calls the core and turns errors into text.
//! Every command is `async`, so Tauri runs it off the main thread: a slow SMB share, SSH call or database lock
//! must not freeze the window.

use crate::AppState;
use binder_core::config::{Config, PrinterConfig};
use binder_core::printer::PrinterCommand;
use crate::printer::{self, PrinterView};
use binder_core::model::*;
use binder_core::notes::{self, DoneItem, DoneLine};
use binder_core::remote::SshRemote;
use binder_core::repo::RepoStatus;
use binder_core::server::ServerStatus;
use binder_core::agent::{self, Access, AgentRun};
use binder_core::cloud::{self, Conflict, Space};
use binder_core::cloud_fast::Fast;
use binder_core::shell::SshShell;
use crate::apps::{self, AppChoice};
use crate::transfers::Unit;
use std::sync::atomic::AtomicBool;
use binder_core::github::{self, GhCli, HiddenRepo, SyncReport};
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

/// The project with its note in its vault folder, moved there or made from the template first when needed.
fn note_for(db: &mut Db, vault: &Path, id: i64) -> Result<Project, Error> {
    binder_core::session::note_for(db, vault, id, &today())?;
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
    state.db.lock().unwrap().delete_project(id, now()).map_err(text)
}

#[tauri::command]
pub async fn hidden_repos(state: State<'_, AppState>) -> Res<Vec<HiddenRepo>> {
    state.db.lock().unwrap().list_hidden_repos().map_err(text)
}

#[tauri::command]
pub async fn unhide_repo(state: State<'_, AppState>, name_with_owner: String) -> Res<()> {
    state.db.lock().unwrap().unhide_repo(&name_with_owner).map_err(text)
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
    let mut db = state.db.lock().unwrap();
    let p = note_for(&mut db, &vault, project_id).map_err(self::text)?;
    let note = p.note.expect("note_for sets the note");
    let at = chrono::Local::now();
    let (date, time) = (at.format("%Y-%m-%d").to_string(), at.format("%H:%M").to_string());
    binder_core::session::write_line(&mut db, Path::new(&note), &p.name, &date, Some(&time), &text).map_err(self::text)
}

#[tauri::command]
pub async fn read_done(state: State<'_, AppState>, project_id: i64) -> Res<Vec<DoneLine>> {
    let p = state.db.lock().unwrap().get_project(project_id).map_err(text)?;
    match p.note {
        Some(note) => notes::read_history(Path::new(&note)).map_err(text),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub async fn done_today(state: State<'_, AppState>) -> Res<Vec<DoneItem>> {
    let projects = state.db.lock().unwrap().list_projects().map_err(text)?;
    Ok(notes::done_on(&projects, &today()))
}

/// When a done line was last written, in Unix seconds. The overview refreshes Done today when it changes.
#[tauri::command]
pub async fn last_done(state: State<'_, AppState>) -> Res<Option<i64>> {
    let value = state.db.lock().unwrap().meta("last_done").map_err(text)?;
    Ok(value.and_then(|v| v.parse().ok()))
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

/// Points the hooks at this app's binder-cli when they run another one (an old build, a moved app). Only the app
/// in /Applications does this, so a dev build never takes the hooks over.
pub fn repair_hooks() {
    let (Ok(settings), Ok(exe)) = (claude_settings(), std::env::current_exe()) else { return };
    let cli = exe.with_file_name("binder-cli");
    if hooks::should_repair(&exe) && cli.is_file() && hooks::hook_cli(&settings).as_deref() != Some(cli.as_path()) {
        let _ = hooks::install(&settings, &cli);
    }
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

/// Whether a process on this Mac is still running.
fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only asks whether the process exists; nothing is sent.
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[tauri::command]
pub async fn cloud_list(state: State<'_, AppState>, rel: String, show_hidden: bool) -> Res<Vec<cloud::Entry>> {
    let root = cloud_root(&state)?;
    // Copies that died with an earlier run of Binder leave partials; they go to the trash as unfinished uploads.
    let _ = cloud::sweep_partials(&root, &rel, &alive);
    cloud::list(&root, &rel, show_hidden).map_err(text)
}

#[tauri::command]
pub async fn cloud_mkdir(state: State<'_, AppState>, rel_dir: String, name: String) -> Res<()> {
    cloud::mkdir(&cloud_root(&state)?, &rel_dir, &name).map(|_| ()).map_err(text)
}

#[tauri::command]
pub async fn cloud_rename(state: State<'_, AppState>, rel: String, new_name: String) -> Res<()> {
    cloud::rename(&cloud_root(&state)?, &rel, &new_name).map(|_| ()).map_err(text)
}

/// Moves an item. A move inside one disk is a rename; one onto another disk (the SD card) becomes a transfer,
/// whose id comes back.
#[tauri::command]
pub async fn cloud_move(app: tauri::AppHandle, state: State<'_, AppState>, rel: String, dest_dir_rel: String) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    match cloud::move_to(&root, &rel, &dest_dir_rel) {
        Ok(_) => Ok(vec![]),
        Err(Error::CrossDisk) => {
            let fast = fast_path(&state);
            let name = rel.rsplit('/').next().unwrap_or_default().to_string();
            Ok(vec![job(&app, &state, name, Unit::Bytes, move |cancel, progress| match &fast {
                Some(f) => f.with(|f| f.move_across(&rel, &dest_dir_rel, cancel, progress)).map(|_| ()),
                None => cloud::move_across(&root, &rel, &dest_dir_rel, cancel, progress).map(|_| ()),
            })])
        }
        Err(e) => Err(text(e)),
    }
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

/// Opens a file in its default app. A zip is extracted beside it on the server instead, as a transfer.
#[tauri::command]
pub async fn cloud_open(app: tauri::AppHandle, state: State<'_, AppState>, rel: String) -> Res<Vec<u64>> {
    let path = cloud::resolve(&cloud_root(&state)?, &rel).map_err(text)?;
    if path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        return cloud_extract(app, state, rel).await;
    }
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(vec![])
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

/// The fast path's parts, owned, so a transfer thread can build a `Fast` from them.
#[derive(Clone)]
pub struct FastParts {
    shell: std::sync::Arc<SshShell>,
    root: std::path::PathBuf,
    remote: String,
}

impl FastParts {
    pub fn with<T>(&self, f: impl FnOnce(&Fast) -> T) -> T {
        f(&Fast { shell: self.shell.as_ref(), root: &self.root, remote_root: &self.remote, gnu_tar: true })
    }
}

/// The fast path when it's on: a server is set, and the first check this run showed the server sees the same
/// folder as the mount. A failed check turns it off until the next start.
fn fast_path(state: &State<AppState>) -> Option<FastParts> {
    let (server, root, remote) = {
        let c = state.config.lock().unwrap();
        (c.server.clone()?, c.cloud.clone(), c.cloud_remote.clone())
    };
    let parts = FastParts { shell: std::sync::Arc::new(SshShell { target: server, program: "ssh".into() }), root, remote };
    let mut known = state.fast.lock().unwrap();
    let on = *known.get_or_insert_with(|| parts.with(|f| f.check()));
    on.then_some(parts)
}

/// Starts one transfer that reports to the tray.
fn job<W>(app: &tauri::AppHandle, state: &State<AppState>, name: String, unit: Unit, work: W) -> u64
where
    W: FnOnce(&AtomicBool, &mut dyn FnMut(u64, u64)) -> Result<(), Error> + Send + 'static,
{
    let app = app.clone();
    state.transfers.spawn(name, unit, move |e| { let _ = app.emit("cloud://transfer", e); }, work)
}

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The relative path of an item inside Cloud, from its path on the mount.
fn rel_of(root: &Path, path: &Path) -> Res<String> {
    path.strip_prefix(root).map(|p| p.to_string_lossy().into_owned()).map_err(|_| "That path is outside Cloud.".to_string())
}

/// Copies files or folders from the Mac into a Cloud folder. Folders go through the fast path when it's on.
#[tauri::command]
pub async fn cloud_upload(app: tauri::AppHandle, state: State<'_, AppState>, paths: Vec<String>, dest_rel: String, conflict: Conflict) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let dest = cloud::resolve(&root, &dest_rel).map_err(text)?;
    if !dest.is_dir() {
        return Err("Upload into a folder.".into());
    }
    let fast = if paths.iter().any(|p| Path::new(p).is_dir()) { fast_path(&state) } else { None };
    Ok(paths
        .into_iter()
        .map(|p| {
            let src = std::path::PathBuf::from(p);
            let (root, dest, dest_rel) = (root.clone(), dest.clone(), dest_rel.clone());
            let fast = fast.clone().filter(|_| src.is_dir());
            job(&app, &state, name_of(&src), Unit::Bytes, move |cancel, progress| match fast {
                Some(f) => f.with(|f| f.upload_dir(&src, &dest_rel, conflict, cancel, progress)).map(|_| ()),
                None => cloud::copy_into(&src, &dest, conflict, Some(&root), cancel, progress).map(|_| ()),
            })
        })
        .collect())
}

/// Copies Cloud items into a folder on the Mac. Folders come through the fast path when it's on.
#[tauri::command]
pub async fn cloud_download(app: tauri::AppHandle, state: State<'_, AppState>, rels: Vec<String>, dest_dir: String, conflict: Conflict) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let dest = std::path::PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err("Save into a folder.".into());
    }
    let srcs = rels.iter().map(|r| cloud::resolve(&root, r)).collect::<Result<Vec<_>, _>>().map_err(text)?;
    let fast = if srcs.iter().any(|p| p.is_dir()) { fast_path(&state) } else { None };
    Ok(rels
        .into_iter()
        .zip(srcs)
        .map(|(rel, src)| {
            let dest = dest.clone();
            let fast = fast.clone().filter(|_| src.is_dir());
            job(&app, &state, name_of(&src), Unit::Bytes, move |cancel, progress| match fast {
                Some(f) => f.with(|f| f.download_dir(&rel, &dest, conflict, cancel, progress)).map(|_| ()),
                None => cloud::copy_into(&src, &dest, conflict, None, cancel, progress).map(|_| ()),
            })
        })
        .collect())
}

/// Copies Cloud items into a Cloud folder, as Paste after Copy does.
#[tauri::command]
pub async fn cloud_copy(app: tauri::AppHandle, state: State<'_, AppState>, rels: Vec<String>, dest_dir_rel: String, conflict: Conflict) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let dest = cloud::resolve(&root, &dest_dir_rel).map_err(text)?;
    let jobs = rels.into_iter().map(|rel| cloud::resolve(&root, &rel).map(|src| (rel, src))).collect::<Result<Vec<_>, _>>().map_err(text)?;
    Ok(copy_jobs(&app, &state, &root, jobs.into_iter().map(|(rel, src)| (rel, src.clone(), dest_dir_rel.clone(), dest.clone(), name_of(&src), conflict)).collect()))
}

type CopyJob = (String, std::path::PathBuf, String, std::path::PathBuf, String, Conflict);

/// Each copy as a transfer: `cp -a` on the server when the fast path is on, the chunked SMB copy otherwise.
fn copy_jobs(app: &tauri::AppHandle, state: &State<AppState>, root: &Path, jobs: Vec<CopyJob>) -> Vec<u64> {
    let fast = if jobs.is_empty() { None } else { fast_path(state) };
    jobs.into_iter()
        .map(|(rel, src, dest_rel, dest, name, conflict)| {
            let (root, fast) = (root.to_path_buf(), fast.clone());
            job(app, state, name.clone(), Unit::Bytes, move |cancel, progress| match fast {
                Some(f) => f.with(|f| f.copy(&rel, &dest_rel, &name, conflict, cancel, progress)).map(|_| ()),
                None => cloud::copy_into_as(&src, &dest, &name, conflict, Some(&root), cancel, progress).map(|_| ()),
            })
        })
        .collect()
}

/// A copy of each item beside it, named as Finder names duplicates.
#[tauri::command]
pub async fn cloud_duplicate(app: tauri::AppHandle, state: State<'_, AppState>, rels: Vec<String>) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let mut jobs = vec![];
    for rel in rels {
        let src = cloud::resolve(&root, &rel).map_err(text)?;
        let dir = src.parent().ok_or("Cloud itself can't be duplicated.")?.to_path_buf();
        let name = name_of(&cloud::copy_name(&dir, &name_of(&src)));
        jobs.push((rel, src, rel_of(&root, &dir)?, dir, name, Conflict::KeepBoth));
    }
    Ok(copy_jobs(&app, &state, &root, jobs))
}

/// Zips items from one folder into it: `name.zip` for one, `Archive.zip` for several.
#[tauri::command]
pub async fn cloud_compress(app: tauri::AppHandle, state: State<'_, AppState>, rels: Vec<String>) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    let name = match rels.as_slice() {
        [one] => format!("{}.zip", one.rsplit('/').next().unwrap_or_default()),
        _ => "Archive.zip".into(),
    };
    let fast = fast_path(&state);
    Ok(vec![job(&app, &state, name, Unit::Items, move |cancel, progress| match fast {
        Some(f) => f.with(|f| f.compress(&rels, cancel, progress)).map(|_| ()),
        None => cloud::compress_local(&root, &rels, cancel, progress).map(|_| ()),
    })])
}

/// Unzips into a folder beside the zip, named after it.
#[tauri::command]
pub async fn cloud_extract(app: tauri::AppHandle, state: State<'_, AppState>, rel: String) -> Res<Vec<u64>> {
    let root = cloud_root(&state)?;
    cloud::resolve(&root, &rel).map_err(text)?;
    let name = rel.rsplit('/').next().unwrap_or_default().to_string();
    let fast = fast_path(&state);
    Ok(vec![job(&app, &state, name, Unit::Items, move |cancel, progress| match fast {
        Some(f) => f.with(|f| f.extract(&rel, cancel, progress)).map(|_| ()),
        None => cloud::extract_local(&root, &rel, cancel, progress).map(|_| ()),
    })])
}

#[tauri::command]
pub async fn cloud_info(state: State<'_, AppState>, rel: String) -> Res<cloud::Info> {
    cloud::info(&cloud_root(&state)?, &rel).map_err(text)
}

/// A folder's total bytes and item count, counted on the server when the fast path is on.
#[tauri::command]
pub async fn cloud_size(state: State<'_, AppState>, rel: String) -> Res<(u64, u64)> {
    let root = cloud_root(&state)?;
    match fast_path(&state) {
        Some(f) => f.with(|f| f.size(&rel)).map_err(text),
        None => cloud::tree_size(&root, &rel).map_err(text),
    }
}

/// The size and free space of the disk holding the folder on screen. Only the server can tell the SD card apart.
#[tauri::command]
pub async fn cloud_space(state: State<'_, AppState>, rel: String) -> Res<Space> {
    let root = cloud_root(&state)?;
    if let Some(f) = fast_path(&state) {
        if let Ok(s) = f.with(|f| f.space(&rel)) {
            return Ok(s);
        }
    }
    cloud::space(&cloud::resolve(&root, &rel).map_err(text)?).map_err(text)
}

/// `"ssh"` when the fast path is on, `"smb"` otherwise.
#[tauri::command]
pub async fn cloud_fast_path(state: State<'_, AppState>) -> Res<String> {
    cloud_root(&state)?;
    Ok(if fast_path(&state).is_some() { "ssh" } else { "smb" }.into())
}

/// Shows Quick Look for an item, or closes it when it's open.
#[tauri::command]
pub async fn cloud_quicklook(state: State<'_, AppState>, rel: String) -> Res<()> {
    let path = cloud::resolve(&cloud_root(&state)?, &rel).map_err(text)?;
    let mut open = state.quicklook.lock().unwrap();
    if let Some(mut child) = open.take() {
        if matches!(child.try_wait(), Ok(None)) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
    }
    let child = std::process::Command::new("/usr/bin/qlmanage")
        .arg("-p")
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    *open = Some(child);
    Ok(())
}

#[tauri::command]
pub async fn cloud_apps_for(state: State<'_, AppState>, rel: String) -> Res<Vec<AppChoice>> {
    let path = cloud::resolve(&cloud_root(&state)?, &rel).map_err(text)?;
    Ok(apps::apps_for(&path))
}

#[tauri::command]
pub async fn cloud_open_with(state: State<'_, AppState>, rel: String, app: String) -> Res<()> {
    let path = cloud::resolve(&cloud_root(&state)?, &rel).map_err(text)?;
    if !apps::is_app(Path::new(&app)) {
        return Err("That isn't an app.".into());
    }
    let status = std::process::Command::new("/usr/bin/open").arg("-a").arg(&app).arg("--").arg(&path).status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("The app couldn't open {}.", name_of(&path))) }
}

#[tauri::command]
pub async fn cloud_cancel(state: State<'_, AppState>, id: u64) -> Res<()> {
    state.transfers.cancel(id);
    Ok(())
}
