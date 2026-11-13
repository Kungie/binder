//! Binder's helper for Claude Code hooks. `hook` must return fast and print nothing, so the model call runs in a
//! detached `summarize` process.

use binder_core::config::Config;
use binder_core::session::{self, ClaudeSummarizer, HookAction, HookInput};
use binder_core::{Db, Result};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn data_dir() -> PathBuf {
    home().join("Library/Application Support/io.github.kungie.binder")
}

fn hook() -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    let Ok(input) = serde_json::from_str::<HookInput>(&stdin) else { return Ok(()) };
    let mut db = Db::open(&data_dir().join("binder.db"))?;
    if let HookAction::Summarize { session_id } = session::on_hook(&mut db, &input, chrono::Utc::now().timestamp())? {
        // Its own process group, so Claude Code ending the hook doesn't end the summary.
        Command::new(std::env::current_exe()?)
            .args(["summarize", &session_id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()?;
    }
    Ok(())
}

/// `claude` from the PATH the hook inherited, or where its installers put it.
fn claude() -> PathBuf {
    let found = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .chain([home().join(".local/bin"), home().join(".claude/local"), PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")])
        .map(|dir| dir.join("claude"))
        .find(|p| p.is_file());
    found.unwrap_or_else(|| PathBuf::from("claude"))
}

fn summarize(session_id: &str) -> Result<()> {
    let config = Config::load(&data_dir().join("config.toml"))?;
    let mut db = Db::open(&data_dir().join("binder.db"))?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    session::flush_pending(&mut db, &config.vault)?;
    let summarizer = ClaudeSummarizer { claude: claude(), scratch: home().join("Library/Caches/Binder/scratch") };
    session::summarize(&mut db, &config.vault, &summarizer, session_id, &today)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["hook", _event] => hook(),
        ["summarize", session_id] => summarize(session_id),
        _ => {
            eprintln!("usage: binder-cli hook <stop|precompact|session-end> | binder-cli summarize <session>");
            std::process::exit(2);
        }
    };
    // A hook never fails the session: errors stay quiet, and the exit code is 0.
    let _ = result;
}
