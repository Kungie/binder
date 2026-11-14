//! Updates from GitHub releases through Tauri's updater plugin: the check, the download with its
//! signature check, and the restart into the new version.

use crate::AppState;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    /// False in development builds, which never update.
    pub updates: bool,
}

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InstallProgress {
    Downloading { percent: Option<u8> },
    Installing,
}

/// How much of the download is done, when its size is known.
pub fn percent(done: u64, total: Option<u64>) -> Option<u8> {
    match total {
        Some(t) if t > 0 => Some((done.saturating_mul(100) / t).min(100) as u8),
        _ => None,
    }
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo { version: app.package_info().version.to_string(), updates: !cfg!(debug_assertions) }
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    if cfg!(debug_assertions) {
        return Err("Updates are off in development builds.".into());
    }
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo { version: u.version, notes: u.body.unwrap_or_default() }))
}

/// Downloads the update, checks its signature against the key in `tauri.conf.json`, replaces the app and
/// restarts into it. Refuses while a Cloud copy runs, so no copy ends halfway.
#[tauri::command]
pub async fn update_install(app: AppHandle, state: State<'_, AppState>, progress: Channel<InstallProgress>) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Updates are off in development builds.".into());
    }
    if state.transfers.running() > 0 {
        return Err("Wait for the Cloud transfer to finish.".into());
    }
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("No update.")?;
    let mut done = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                done += chunk as u64;
                let _ = progress.send(InstallProgress::Downloading { percent: percent(done, total) });
            },
            || {
                let _ = progress.send(InstallProgress::Installing);
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    // As quitting does: the local ssh processes end, the tmux sessions on the server stay.
    state.terminals.close_all();
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_needs_a_total() {
        assert_eq!(percent(5, None), None);
        assert_eq!(percent(5, Some(0)), None);
    }

    #[test]
    fn percent_rounds_down_and_caps() {
        assert_eq!(percent(421, Some(1000)), Some(42));
        assert_eq!(percent(1200, Some(1000)), Some(100));
    }

    #[test]
    fn progress_serializes_for_the_window() {
        assert_eq!(serde_json::to_value(InstallProgress::Downloading { percent: Some(42) }).unwrap(), serde_json::json!({"kind": "downloading", "percent": 42}));
        assert_eq!(serde_json::to_value(InstallProgress::Installing).unwrap(), serde_json::json!({"kind": "installing"}));
    }
}
