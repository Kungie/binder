//! Binder's core: tasks, projects, the config file and the project notes.
//! It knows nothing about Tauri, so everything here is tested with `cargo test`.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Toml(String),
    #[error("no {0} with id {1}")]
    NotFound(&'static str, i64),
    #[error("{0}")]
    Invalid(String),
    #[error("Vault not connected: {}", .0.display())]
    VaultMissing(PathBuf),
    #[error("{0}")]
    Remote(String),
}

pub mod agent;
pub mod config;
pub mod db;
pub mod devices;
pub mod model;
pub mod notes;
pub mod printer;
pub mod projects;
pub mod remote;
pub mod repo;
pub mod server;
pub mod tasks;

pub use db::Db;
