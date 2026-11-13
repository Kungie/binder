//! `config.toml`: settings that are the owner's, kept out of the database and out of the repo.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Where the Obsidian vault is mounted.
    pub vault: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Config { vault: PathBuf::from("/Volumes/vault") }
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
