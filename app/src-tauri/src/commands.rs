//! The commands the interface calls. Each one locks what it needs, calls the core and turns errors into text.
//! Every command is `async`, so Tauri runs it off the main thread: a slow SMB share, SSH call or database lock
//! must not freeze the window.

use crate::AppState;
use binder_core::config::{Config, PrinterConfig};
use binder_core::printer::PrinterCommand;
use crate::printer::{self, PrinterView};
use binder_core::model::*;
use binder_core::notes::{self, DoneItem, DoneLine};
use binder_core::remote::{Remote, SshRemote};
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
use binder_core::gh_work;
use binder_core::{Db, Error, devices, hooks, repo, server};
use tauri::{Emitter, Manager};
use std::path::Path;
use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody};
use binder_core::agents::{self, Pane};
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
    let wrote = binder_core::session::write_line(&mut db, Path::new(&note), &p.name, &date, Some(&time), &text).map_err(self::text)?;
    if wrote {
        binder_core::session::remember(&mut db, project_id, &date, Some(&time), &text).map_err(self::text)?;
    }
    Ok(wrote)
}

#[tauri::command]
pub async fn read_done(state: State<'_, AppState>, project_id: i64) -> Res<Vec<DoneLine>> {
    let db = state.db.lock().unwrap();
    let p = db.get_project(project_id).map_err(text)?;
    binder_core::session::history(&db, &p).map_err(text)
}

#[tauri::command]
pub async fn done_today(state: State<'_, AppState>) -> Res<Vec<DoneItem>> {
    let db = state.db.lock().unwrap();
    let projects = db.list_projects().map_err(text)?;
    binder_core::session::done_today(&db, &projects, &today()).map_err(text)
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
    Ok(vault_here(&vault(&state)))
}

/// Mounts the vault share now, with the password saved in the Keychain.
#[tauri::command]
pub async fn vault_connect(state: State<'_, AppState>) -> Res<bool> {
    let share = state.config.lock().unwrap().vault_share_address().ok_or_else(|| "Set the Cloud share address in Settings first.".to_string())?;
    crate::shares::connect(&share).map_err(|e| e.text().to_string())?;
    Ok(vault_here(&vault(&state)))
}

fn server_target(state: &State<AppState>) -> Res<String> {
    state.config.lock().unwrap().server.clone().filter(|s| !s.trim().is_empty()).ok_or_else(|| "No server set.".to_string())
}

/// One SSH call; the window keeps working while it runs.
#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Res<ServerStatus> {
    let target = server_target(&state)?;
    server::fetch(&SshRemote::new(target), now(), Some(&sd_path(&state))).map_err(text)
}

#[tauri::command]
pub async fn container_logs(state: State<'_, AppState>, name: String) -> Res<String> {
    let target = server_target(&state)?;
    server::logs(&SshRemote::new(target), &name).map_err(text)
}

#[tauri::command]
pub async fn restart_container(state: State<'_, AppState>, name: String) -> Res<()> {
    let target = server_target(&state)?;
    server::restart_and_log(&state.db, &SshRemote::new(target), &name, now()).map_err(text)
}

#[tauri::command]
pub async fn sites(state: State<'_, AppState>) -> Res<Vec<binder_core::sites::Site>> {
    let target = server_target(&state)?;
    let saved = state.config.lock().unwrap().site_addresses.clone();
    binder_core::sites::read(&SshRemote::new(target), &saved).map_err(text)
}

/// Asks the address from the Mac with the system curl, the url as one argument and never through a shell.
#[tauri::command]
pub async fn site_check(url: String) -> Res<binder_core::sites::SiteCheck> {
    check_site(&url).map(|(c, _)| c)
}

/// One https check from the Mac and how long the answer took, in milliseconds.
pub fn check_site(url: &str) -> Res<(binder_core::sites::SiteCheck, Option<u32>)> {
    if !url.starts_with("https://") {
        return Err("Only https addresses are checked.".into());
    }
    let out = std::process::Command::new("/usr/bin/curl")
        .args(["-sS", "-o", "/dev/null", "-D", "-", "-w", "\n--binder:time--%{time_total}", "--max-time", "5", "--proto", "=https", "--proto-redir", "=https", "-L", "--", url])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let error = if out.status.code() == Some(28) { "No answer in 5 s".to_string() } else { "No answer".to_string() };
        return Ok((binder_core::sites::SiteCheck { status: None, frameable: false, error: Some(error) }, None));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (headers, time) = text.rsplit_once("\n--binder:time--").unwrap_or((&text, ""));
    let ms = time.trim().parse::<f64>().ok().map(|s| (s * 1000.0).round() as u32);
    Ok((binder_core::sites::parse_check(headers), ms))
}

/// A site's last week: 28 bars of six hours, the share of good checks and the last answer time.
#[tauri::command]
pub async fn site_uptime(state: State<'_, AppState>, name: String) -> Res<binder_core::uptime::Summary> {
    binder_core::uptime::summary(&state.db.lock().unwrap(), &name, now(), 7 * 86_400, 28).map_err(text)
}

/// The quick add window made a task: the main window reloads its tasks.
#[tauri::command]
pub async fn quick_added(app: tauri::AppHandle) -> Res<()> {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.hide();
        crate::tray::quick_hidden();
    }
    app.emit("tasks://changed", ()).map_err(|e| e.to_string())
}

/// Hides the quick add window without a task.
#[tauri::command]
pub async fn quick_close(app: tauri::AppHandle) -> Res<()> {
    if let Some(w) = app.get_webview_window("quick") {
        let _ = w.hide();
        crate::tray::quick_hidden();
    }
    Ok(())
}

/// The page of a site that refuses to be framed, fetched from the Mac (https only, at most 3 MB) and made into a
/// still snapshot: it is shown in a frame that runs no scripts.
#[tauri::command]
pub async fn site_snapshot(url: String) -> Res<String> {
    if !url.starts_with("https://") {
        return Err("Only https addresses are read.".into());
    }
    let out = std::process::Command::new("/usr/bin/curl")
        .args(["-sS", "--max-time", "8", "--max-filesize", "3000000", "--proto", "=https", "--proto-redir", "=https", "-L", "-w", "\n--binder:url--%{url_effective}", "--", &url])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("No answer".into());
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let (html, last) = body.rsplit_once("\n--binder:url--").unwrap_or((&body, &url));
    let last = if last.starts_with("https://") { last } else { &url };
    Ok(binder_core::sites::snapshot(html, last))
}

#[tauri::command]
pub async fn set_site_address(state: State<'_, AppState>, name: String, url: String) -> Res<String> {
    let url = url.trim();
    let host = url.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/');
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("Give it an address like example.com.".into());
    }
    let kept = format!("https://{host}");
    let mut config = state.config.lock().unwrap();
    config.site_addresses.insert(name, kept.clone());
    config.save(&state.config_path).map_err(text)?;
    Ok(kept)
}

/// Opens an https link in the browser; nothing else.
#[tauri::command]
pub async fn open_link(app: tauri::AppHandle, url: String) -> Res<()> {
    if !url.starts_with("https://") {
        return Err("Only https links open.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

fn gh() -> Res<GhCli> {
    GhCli::find().ok_or_else(|| "Install gh and sign in with gh auth login.".to_string())
}

/// The project a repo belongs to; the GitHub screen only acts on Binder's own projects.
fn project_for_repo(state: &State<AppState>, repo: &str) -> Res<Project> {
    state
        .db
        .lock()
        .unwrap()
        .list_projects()
        .map_err(text)?
        .into_iter()
        .find(|p| p.github.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(repo)))
        .ok_or_else(|| "That repo isn't one of your projects.".to_string())
}

/// The project with its repo and agent folder, for the agent's GitHub work.
fn agent_project(state: &State<AppState>, project_id: i64, repo: &str) -> Res<(Project, String)> {
    let p = project_for_repo(state, repo)?;
    if p.id != project_id {
        return Err("That repo belongs to another project.".into());
    }
    let dir = p.agent_dir.clone().filter(|d| !d.trim().is_empty()).ok_or_else(|| "Set the project's agent folder first.".to_string())?;
    Ok((p, dir))
}

#[tauri::command]
pub async fn gh_list(state: State<'_, AppState>, filter: gh_work::Filter, project_id: Option<i64>) -> Res<gh_work::Lists> {
    use gh_work::GhWork;
    let repos: Vec<String> = state
        .db
        .lock()
        .unwrap()
        .list_projects()
        .map_err(text)?
        .into_iter()
        .filter(|p| !p.archived && project_id.is_none_or(|id| p.id == id))
        .filter_map(|p| p.github.map(|g| g.to_lowercase()))
        .collect();
    let mut l = gh()?.lists().map_err(text)?;
    l.pulls.retain(|p| repos.contains(&p.repo.to_lowercase()));
    l.issues.retain(|i| repos.contains(&i.repo.to_lowercase()));
    let (pulls, issues) = match filter {
        gh_work::Filter::NeedsYou => gh_work::needs_you(&l.pulls, &l.issues, &l.login),
        gh_work::Filter::Mine => gh_work::mine(&l.pulls, &l.issues, &l.login),
        gh_work::Filter::AllOpen => (l.pulls, l.issues),
    };
    Ok(gh_work::Lists { login: l.login, pulls, issues })
}

#[tauri::command]
pub async fn gh_detail(state: State<'_, AppState>, repo: String, number: i64, pull: bool) -> Res<gh_work::Detail> {
    use gh_work::GhWork;
    project_for_repo(&state, &repo)?;
    gh()?.detail(&repo, number, pull).map_err(text)
}

#[tauri::command]
pub async fn gh_act(state: State<'_, AppState>, repo: String, number: i64, pull: bool, action: gh_work::Action) -> Res<()> {
    use gh_work::GhWork;
    project_for_repo(&state, &repo)?;
    gh()?.act(&repo, number, pull, &action).map_err(text)
}

#[tauri::command]
pub async fn gh_new_issue(state: State<'_, AppState>, project_id: i64, title: String, body: String) -> Res<i64> {
    use gh_work::GhWork;
    let repo = state.db.lock().unwrap().get_project(project_id).map_err(text)?.github.ok_or_else(|| "This project isn't on GitHub.".to_string())?;
    if title.trim().is_empty() {
        return Err("An issue needs a title.".into());
    }
    gh()?.new_issue(&repo, title.trim(), &body).map_err(text)
}

#[derive(serde::Serialize)]
pub struct AgentReady {
    pub gh: bool,
    pub folder: bool,
}

#[tauri::command]
pub async fn gh_agent_ready(state: State<'_, AppState>, project_id: i64) -> Res<AgentReady> {
    let folder = state.db.lock().unwrap().get_project(project_id).map_err(text)?.agent_dir.is_some_and(|d| !d.trim().is_empty());
    let gh = server_target(&state).ok().is_some_and(|t| agent::server_gh(&SshRemote::new(t)).unwrap_or(false));
    Ok(AgentReady { gh, folder })
}

#[tauri::command]
pub async fn gh_review(state: State<'_, AppState>, project_id: i64, repo: String, number: i64) -> Res<AgentRun> {
    use gh_work::GhWork;
    let (p, dir) = agent_project(&state, project_id, &repo)?;
    let target = server_target(&state)?;
    let gh = gh()?;
    let detail = gh.detail(&repo, number, true).map_err(text)?;
    let diff = gh.diff(&repo, number).map_err(text)?;
    let task = gh_work::review_task(&mut state.db.lock().unwrap(), p.id, &repo, number, &detail.title, now()).map_err(text)?;
    let prompt = gh_work::review_prompt(&detail.title, &detail.body, &diff);
    start_or_undo(&state, &SshRemote::new(target), task.id, true, &dir, Access::ReadOnly, &prompt)
}

/// The review a run wrote once it ended well; `None` while it runs.
#[tauri::command]
pub async fn gh_review_draft(state: State<'_, AppState>, run: String) -> Res<Option<String>> {
    let remote = SshRemote::new(server_target(&state)?);
    let r = agent::poll_run(&state.db, &remote, &run, now()).map_err(text)?;
    match (r.ended, r.exit_code) {
        (None, _) => Ok(None),
        (Some(_), Some(0)) => agent::output(&remote, &run).map(Some).map_err(text),
        _ => Err("The agent's review didn't finish. Its run on the task says why.".into()),
    }
}

#[tauri::command]
pub async fn gh_work(state: State<'_, AppState>, project_id: i64, repo: String, number: i64, title: String) -> Res<AgentRun> {
    use gh_work::GhWork;
    let (p, dir) = agent_project(&state, project_id, &repo)?;
    let remote = SshRemote::new(server_target(&state)?);
    if !agent::server_gh(&remote).unwrap_or(false) {
        return Err("Sign in to gh on the server to let the agent push.".into());
    }
    let detail = gh()?.detail(&repo, number, false).map_err(text)?;
    let issue = github::Issue { repo: repo.clone(), number, title: title.clone(), body: detail.body.clone(), url: detail.url.clone() };
    let task = gh_work::issue_task(&mut state.db.lock().unwrap(), p.id, &issue, now()).map_err(text)?;
    let prompt = gh_work::work_prompt(&task.key, &repo, number, &title, &detail.body);
    start_or_undo(&state, &remote, task.id, false, &dir, Access::Full, &prompt)
}

#[tauri::command]
pub async fn gh_fix(state: State<'_, AppState>, project_id: i64, repo: String, number: i64) -> Res<AgentRun> {
    use gh_work::GhWork;
    let (p, dir) = agent_project(&state, project_id, &repo)?;
    let remote = SshRemote::new(server_target(&state)?);
    if !agent::server_gh(&remote).unwrap_or(false) {
        return Err("Sign in to gh on the server to let the agent push.".into());
    }
    let detail = gh()?.detail(&repo, number, true).map_err(text)?;
    let branch = detail.head.clone().ok_or_else(|| "That pull request has no branch.".to_string())?;
    let task = state
        .db
        .lock()
        .unwrap()
        .create_task(NewTask { title: format!("Fix #{number}: {}", detail.title), project_id: Some(p.id), assignee: Some(Assignee::Agent), ..Default::default() }, now())
        .map_err(text)?;
    let prompt = gh_work::fix_prompt(&repo, number, &branch);
    start_or_undo(&state, &remote, task.id, true, &dir, Access::Full, &prompt)
}

/// Starts the agent on a task; a start that fails gives the task back (see `gh_work::undo_start`).
fn start_or_undo(state: &State<AppState>, remote: &SshRemote, task_id: i64, made_for_it: bool, dir: &str, access: Access, prompt: &str) -> Res<AgentRun> {
    agent::start_run_with(&state.db, remote, task_id, dir, access, prompt, now()).map_err(|e| {
        let _ = gh_work::undo_start(&mut state.db.lock().unwrap(), task_id, made_for_it, now());
        text(e)
    })
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
        .map(|c| state.printers.view(&c.serial).unwrap_or(PrinterView { config: c, state: Default::default(), online: false, error: None, problems: vec![] }))
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

/// The last 50 prints, newest first.
#[tauri::command]
pub async fn printer_history(state: State<'_, AppState>) -> Res<Vec<binder_core::printer::history::Print>> {
    binder_core::printer::history::list(&state.db.lock().unwrap(), 50).map_err(text)
}

/// Streams a printer's camera as raw JPEG frames until `printer_camera_stop`.
#[tauri::command]
pub async fn printer_camera(app: tauri::AppHandle, serial: String, frames: Channel<InvokeResponseBody>) -> Res<()> {
    printer::camera_start(&app, &serial, move |jpeg| frames.send(InvokeResponseBody::Raw(jpeg)).is_ok())
}

#[tauri::command]
pub async fn printer_camera_stop(serial: String) -> Res<()> {
    printer::camera_stop(&serial);
    Ok(())
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

/// One sync at a time: the start-up sync and a manual one would both add the same new issue.
static SYNCING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Brings GitHub repos in on its own database connection, so the app's lock isn't held while `gh` runs.
pub fn sync_github(data_dir: &Path) -> Result<SyncReport, Error> {
    let _one = SYNCING.lock().unwrap_or_else(|e| e.into_inner());
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
    let result = sync_github(data_dir);
    if result.as_ref().is_ok_and(|r| r.issues_added + r.issues_closed > 0) {
        let _ = app.emit("tasks://changed", ());
        let _ = app.emit("brief://changed", ());
    }
    let _ = app.emit("projects://changed", GithubSync { at: now(), error: result.err().map(text) });
}

/// What the brief command sends: where it starts, whether it shows now, and its lines.
#[derive(serde::Serialize)]
pub struct BriefView {
    pub since: i64,
    pub shown: bool,
    pub items: Vec<binder_core::brief::BriefItem>,
}

fn local(t: i64) -> Option<chrono::NaiveDateTime> {
    use chrono::TimeZone;
    chrono::Local.timestamp_opt(t, 0).single().map(|d| d.naive_local())
}

#[tauri::command]
pub async fn brief(state: State<'_, AppState>) -> Res<BriefView> {
    use binder_core::brief as b;
    let db = state.db.lock().unwrap();
    let now = now();
    let since = b::since(&db, now).map_err(text)?;
    let dismissed = b::dismissed_at(&db).map_err(text)?.and_then(local);
    let shown = local(now).is_some_and(|n| b::shown(n, dismissed));
    Ok(BriefView { since, shown, items: b::items(&db, since, now + 1).map_err(text)? })
}

#[tauri::command]
pub async fn dismiss_brief(state: State<'_, AppState>) -> Res<()> {
    binder_core::brief::dismiss(&mut state.db.lock().unwrap(), now()).map_err(text)
}

#[tauri::command]
pub async fn github_sync(app: tauri::AppHandle, state: State<'_, AppState>) -> Res<SyncReport> {
    let dir = state.config_path.parent().expect("config sits in the data folder").to_path_buf();
    let report = sync_github(&dir).map_err(text)?;
    if report.issues_added + report.issues_closed > 0 {
        let _ = app.emit("brief://changed", ());
    }
    Ok(report)
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

/// The vault is there: a folder under /Volumes only when a share is mounted on it, any other folder when it exists.
pub fn vault_here(path: &Path) -> bool {
    if path.starts_with("/Volumes") { mounted(path) } else { path.is_dir() }
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

#[derive(Clone, serde::Serialize)]
struct TermExit {
    session: String,
    message: String,
}

/// Starts the pane's `ssh` into its tmux session, or hands a running one's output to this window.
fn term_start(app: tauri::AppHandle, state: &State<AppState>, pane: &Pane, cols: u16, rows: u16, output: Channel<InvokeResponseBody>) -> Res<()> {
    // A device's shell goes to its own host; ssh_args takes it from the pane.
    let target = if matches!(pane, Pane::Ssh { .. }) { String::new() } else { server_target(state)? };
    let session = pane.session();
    let exit_session = session.clone();
    state.terminals.open(
        &session,
        "ssh",
        &agents::ssh_args(&target, pane).map_err(text)?,
        cols,
        rows,
        move |bytes| {
            let _ = output.send(InvokeResponseBody::Raw(bytes));
        },
        move |message| {
            let _ = app.emit("term-exit", TermExit { session: exit_session, message });
        },
    )?;
    Ok(())
}

#[tauri::command]
pub async fn term_open(app: tauri::AppHandle, state: State<'_, AppState>, pane: Pane, cols: u16, rows: u16, output: Channel<InvokeResponseBody>) -> Res<()> {
    term_start(app, &state, &pane, cols, rows, output)
}

/// The terminals' copies, from a tmux selection or ⌘C.
#[tauri::command]
pub async fn copy_text(text: String) -> Res<()> {
    crate::terminal::pipe_to("/usr/bin/pbcopy", &[], &text)
}

#[tauri::command]
pub async fn term_write(state: State<'_, AppState>, session: String, data: String) -> Res<()> {
    state.terminals.write(&session, data.as_bytes())
}

#[tauri::command]
pub async fn term_resize(state: State<'_, AppState>, session: String, cols: u16, rows: u16) -> Res<()> {
    state.terminals.resize(&session, cols, rows)
}

#[tauri::command]
pub async fn term_reconnect(app: tauri::AppHandle, state: State<'_, AppState>, pane: Pane, cols: u16, rows: u16, output: Channel<InvokeResponseBody>) -> Res<()> {
    state.terminals.close(&pane.session());
    term_start(app, &state, &pane, cols, rows, output)
}

/// Every Binder tmux session on the server and what it's doing.
#[tauri::command]
pub async fn agent_sessions(state: State<'_, AppState>) -> Res<Vec<agents::Session>> {
    let r = remote(&state)?;
    Ok(agents::parse_sessions(&r.run(agents::sessions_script(), None).map_err(text)?))
}

/// Ends a session on the server, and its terminal here.
#[tauri::command]
pub async fn agent_session_close(state: State<'_, AppState>, session: String) -> Res<()> {
    let script = agents::close_script(&session).map_err(text)?;
    state.terminals.close(&session);
    remote(&state)?.run(&script, None).map_err(text)?;
    Ok(())
}

/// The session the Agents screen shows, or none: its alerts stay quiet while Binder is in front.
#[tauri::command]
pub async fn agent_viewing(session: Option<String>) -> Res<()> {
    crate::agents_watch::viewing(session);
    Ok(())
}

#[tauri::command]
pub async fn ollama_models(state: State<'_, AppState>) -> Res<Vec<String>> {
    Ok(agents::parse_models(&remote(&state)?.run(agents::MODELS_SCRIPT, None).map_err(text)?))
}

/// Saves the model and ends the Ollama session, so the next attach runs it.
#[tauri::command]
pub async fn set_ollama_model(state: State<'_, AppState>, model: String) -> Res<()> {
    let model = model.trim().to_string();
    if !agents::valid_model(&model) {
        return Err("That isn't a model name.".into());
    }
    {
        let mut c = state.config.lock().unwrap();
        c.ollama_model = model;
        c.save(&state.config_path).map_err(text)?;
    }
    let session = Pane::Ollama { model: String::new() }.session();
    state.terminals.close(&session);
    remote(&state)?.run(&agents::close_script(&session).map_err(text)?, None).map_err(text)?;
    Ok(())
}

/// Claude Code's tokens on the server and on this Mac together. A server that can't be read counts nothing.
#[tauri::command]
pub async fn claude_usage(state: State<'_, AppState>) -> Res<binder_core::usage::Usage> {
    use binder_core::usage;
    let now = now();
    let midnight = chrono::Local::now().date_naive().and_hms_opt(0, 0, 0).and_then(|d| d.and_local_timezone(chrono::Local).earliest()).map_or(now - 86_400, |d| d.timestamp());
    static MAC: std::sync::Mutex<Option<usage::Cache>> = std::sync::Mutex::new(None);
    let local = {
        let root = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".claude/projects");
        MAC.lock().unwrap().get_or_insert_with(usage::Cache::default).read(&root)
    };
    let mut total = usage::summary(&local, now, midnight);
    if let Ok(r) = remote(&state) {
        if let Ok(out) = r.run(usage::SCRIPT, None) {
            total = total + usage::summary(&usage::parse(&out), now, midnight);
        }
    }
    Ok(total)
}

#[tauri::command]
pub async fn recent_runs(state: State<'_, AppState>) -> Res<Vec<agent::RecentRun>> {
    agent::recent_runs(&state.db.lock().unwrap(), now()).map_err(text)
}

/// The SD card's mount point inside Cloud on the server.
fn sd_path(state: &State<AppState>) -> String {
    format!("{}/SD", state.config.lock().unwrap().cloud_remote.trim_end_matches('/'))
}

/// Whether the crontab line for night tasks is on the server.
#[tauri::command]
pub async fn night_state(state: State<'_, AppState>) -> Res<bool> {
    binder_core::night::cron_state(&remote(&state)?).map_err(text)
}

#[tauri::command]
pub async fn night_setup(state: State<'_, AppState>) -> Res<bool> {
    let r = remote(&state)?;
    binder_core::night::cron_install(&r).map_err(text)?;
    binder_core::night::cron_state(&r).map_err(text)
}

/// Empties the queue first, so nothing waits for a timer that is gone.
#[tauri::command]
pub async fn night_remove(state: State<'_, AppState>) -> Res<bool> {
    let r = remote(&state)?;
    binder_core::night::clear(&state.db, &r).map_err(text)?;
    binder_core::night::cron_remove(&r).map_err(text)?;
    binder_core::night::cron_state(&r).map_err(text)
}

#[tauri::command]
pub async fn server_processes(state: State<'_, AppState>, by_memory: bool) -> Res<Vec<server::TopProcess>> {
    server::processes(&remote(&state)?, by_memory).map_err(text)
}

#[tauri::command]
pub async fn server_folders(state: State<'_, AppState>) -> Res<Vec<server::TopFolder>> {
    let sd = sd_path(&state);
    server::folders(&remote(&state)?, &["/", &sd]).map_err(text)
}

/// The interface sizes Settings offers, in percent.
pub const ZOOMS: [u16; 4] = [100, 110, 125, 150];

/// Scales the whole window to the saved interface size.
pub fn apply_zoom(app: &tauri::AppHandle) {
    let zoom = app.state::<AppState>().config.lock().unwrap().zoom;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_zoom(f64::from(if ZOOMS.contains(&zoom) { zoom } else { 100 }) / 100.0);
    }
}

#[tauri::command]
pub async fn set_zoom(app: tauri::AppHandle, state: State<'_, AppState>, percent: u16) -> Res<()> {
    if !ZOOMS.contains(&percent) {
        return Err("The interface size is 100, 110, 125 or 150%.".into());
    }
    let config = {
        let mut c = state.config.lock().unwrap();
        c.zoom = percent;
        c.clone()
    };
    config.save(&state.config_path).map_err(text)?;
    apply_zoom(&app);
    Ok(())
}

/// The tailnet as the Mac's Tailscale app sees it.
#[tauri::command]
pub async fn tailnet_status() -> Res<binder_core::tailscale::Status> {
    crate::tailnet::status()
}

/// Each device's pings of the last day, by Tailscale id.
#[tauri::command]
pub async fn tailnet_history() -> Res<std::collections::HashMap<String, Vec<crate::tailnet::Sample>>> {
    Ok(crate::tailnet::history())
}

#[tauri::command]
pub async fn tailnet_ping(ip: String) -> Res<Option<binder_core::tailscale::Ping>> {
    crate::tailnet::ping(&ip)
}

/// Sends files to a device with Taildrop.
#[tauri::command]
pub async fn taildrop(target: String, paths: Vec<String>) -> Res<()> {
    crate::tailnet::send(&target, &paths)
}

/// Ends a terminal here only: a device's shell, or the local ssh of a tmux session that stays.
#[tauri::command]
pub async fn term_close(state: State<'_, AppState>, session: String) -> Res<()> {
    state.terminals.close(&session);
    Ok(())
}

#[derive(serde::Serialize)]
pub struct ClaudeLimits {
    /// Whether Binder's status line is in the server's Claude Code settings.
    installed: bool,
    limits: Option<binder_core::limits::Limits>,
}

/// Claude Code's plan limits in percent, as last handed to the status line on the server.
#[tauri::command]
pub async fn claude_limits(state: State<'_, AppState>) -> Res<ClaudeLimits> {
    use binder_core::limits;
    let script = format!("if grep -qs binder-statusline.sh \"$HOME/.claude/settings.json\"; then echo on; else echo off; fi\n{}", limits::READ_SCRIPT);
    let out = remote(&state)?.run(&script, None).map_err(text)?;
    let (head, rest) = out.split_once('\n').unwrap_or((&out, ""));
    Ok(ClaudeLimits { installed: head.trim() == "on", limits: limits::parse(rest, now()) })
}

/// Adds Binder's status line to Claude Code on the server, so the limits can be read.
#[tauri::command]
pub async fn install_claude_limits(state: State<'_, AppState>) -> Res<()> {
    binder_core::limits::install(&remote(&state)?).map_err(text)
}

/// Turns the VPN on through an exit node (remembered as the last one) or off.
#[tauri::command]
pub async fn vpn_set(state: State<'_, AppState>, exit: Option<String>) -> Res<()> {
    crate::tailnet::set_exit_node(exit.as_deref())?;
    if let Some(ip) = exit {
        let mut c = state.config.lock().unwrap();
        c.vpn_last = Some(ip);
        c.save(&state.config_path).map_err(text)?;
    }
    Ok(())
}

/// Whether a server answers over SSH now, for the server list's dots. Any target from the list, checked first.
#[tauri::command]
pub async fn server_reachable(target: String) -> Res<bool> {
    if !binder_core::tailscale::valid_target(&target) {
        return Err("That isn't an SSH target.".into());
    }
    Ok(SshRemote::new(target).run("true\n", None).is_ok())
}
