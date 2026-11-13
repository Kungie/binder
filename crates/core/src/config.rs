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
        Config { vault: PathBuf::from("/Volumes/vault"), server: None, printers: vec![], claude_hooks: false }
    }
}

impl Config {
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
