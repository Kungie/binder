//! `config.toml`: settings that are the owner's, kept out of the database and out of the repo.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Where the Obsidian vault is mounted.
    pub vault: PathBuf,
    /// The home server's SSH target: an alias from `~/.ssh/config` or `user@host`.
    pub server: Option<String>,
    pub printers: Vec<PrinterConfig>,
    /// Whether Binder's Claude Code hooks are installed.
    pub claude_hooks: bool,
    /// Where the Cloud share is mounted.
    pub cloud: PathBuf,
    /// The share's address, for Connect: `smb://host/cloud`.
    pub cloud_share: Option<String>,
    /// The vault share's address, when it isn't the `vault` share beside Cloud's.
    pub vault_share: Option<String>,
    /// The shared folder's path on the server, for the SSH fast path.
    pub cloud_remote: String,
    /// Whether Binder starts the agent's tasks by itself.
    pub pick_up: bool,
    /// Whether Binder watches the server's containers and sends notifications.
    pub watch: bool,
    /// The interface size in percent: 100, 110, 125 or 150.
    pub zoom: u16,
    /// Whether a finished or failed print sends a notification.
    pub print_notify: bool,
    /// Addresses the owner gave sites Binder couldn't find one for, by folder name.
    pub site_addresses: std::collections::BTreeMap<String, String>,
    /// Which widgets Overview shows, and where.
    pub overview: OverviewLayout,
    /// The model the Agents screen's Ollama session runs.
    pub ollama_model: String,
    /// Claude Code sessions the owner made on the Agents screen, in their order.
    pub agents: Vec<SavedAgent>,
    /// Names the owner gave sessions, by tmux session.
    pub agent_names: std::collections::BTreeMap<String, String>,
}

/// A Claude Code session of the owner's: what it is called and the server folder it starts in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SavedAgent {
    pub name: String,
    pub dir: String,
}

/// Overview's two columns, each a list of widget ids from top to bottom, how wide the left one is in percent,
/// and the heights in pixels of the widgets the owner sized (the others fit their content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewLayout {
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub split: u8,
    pub heights: std::collections::BTreeMap<String, u32>,
}

impl Default for OverviewLayout {
    fn default() -> Self {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect();
        OverviewLayout { left: ids(&["numbers", "done_today"]), right: ids(&["server", "cloud", "printer", "open_tasks"]), split: 50, heights: Default::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PrinterConfig {
    pub name: String,
    pub address: String,
    pub serial: String,
    /// SHA-256 of the printer's certificate, pinned on first connect.
    pub cert_sha256: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config { vault: PathBuf::from("/Volumes/vault"), server: None, printers: vec![], claude_hooks: false, cloud: PathBuf::from("/Volumes/cloud"), cloud_share: None, vault_share: None, cloud_remote: "/srv/cloud".into(), pick_up: true, watch: true, zoom: 100, print_notify: true, site_addresses: Default::default(), overview: OverviewLayout::default(), ollama_model: crate::agents::DEFAULT_MODEL.into(), agents: vec![], agent_names: Default::default() }
    }
}

/// An `smb://` address that can go inside a quoted AppleScript string as it is.
pub fn mountable(share: &str) -> bool {
    share.strip_prefix("smb://").is_some_and(|rest| rest.len() > 1 && rest.contains('/') && !rest.contains(['"', '\\', '\n', '\r']))
}

impl Config {
    /// Where the vault share is: the owner's own address, else the `vault` share on Cloud's server.
    pub fn vault_share_address(&self) -> Option<String> {
        if let Some(s) = self.vault_share.as_ref().filter(|s| !s.trim().is_empty()) {
            return Some(s.trim().to_string());
        }
        let cloud = self.cloud_share.as_deref()?.trim().trim_end_matches('/');
        let (host, _) = cloud.rsplit_once('/').filter(|(h, _)| h.len() > "smb://".len())?;
        Some(format!("{host}/vault"))
    }

    /// The saved settings, or the defaults when there's no file yet.
    pub fn load(path: &Path) -> Result<Config> {
        if !path.exists() {
            return Ok(Config::default());
        }
        toml::from_str(&std::fs::read_to_string(path)?).map_err(|e| Error::Toml(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, toml::to_string(self).map_err(|e| Error::Toml(e.to_string()))?)?;
        Ok(())
    }
}
