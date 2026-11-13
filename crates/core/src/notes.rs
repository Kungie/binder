//! Project notes in the Obsidian vault. Binder writes a note once from a template and after that
//! only appends dated lines under `## Done`; every other byte belongs to the owner.

use crate::model::Project;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneLine {
    pub date: String,
    pub text: String,
}

const DONE_HEADING: &str = "## Done";
const MAX_CHARS: usize = 200;

/// `vault/Projects/<name>.md`, with characters a file name or Obsidian link can't hold replaced by `-`.
pub fn note_path(vault: &Path, project_name: &str) -> PathBuf {
    let replaced: String = project_name
        .chars()
        .map(|c| if "/\\:*?\"<>|#^[]".contains(c) || c.is_control() { '-' } else { c })
        .collect();
    let mut collapsed = String::new();
    for c in replaced.chars() {
        if (c == '-' || c == ' ') && collapsed.ends_with(c) {
            continue;
        }
        collapsed.push(c);
    }
    let safe = collapsed.trim_matches(|c| c == ' ' || c == '-');
    vault.join("Projects").join(format!("{}.md", if safe.is_empty() { "Untitled" } else { safe }))
}

fn need_vault(vault: &Path) -> Result<()> {
    if vault.is_dir() { Ok(()) } else { Err(Error::VaultMissing(vault.to_path_buf())) }
}

/// Makes the project's note if it doesn't exist. True when it was made now; an existing note is left alone.
pub fn ensure_note(vault: &Path, p: &Project, today: &str) -> Result<(PathBuf, bool)> {
    need_vault(vault)?;
    let path = note_path(vault, &p.name);
    if path.exists() {
        return Ok((path, false));
    }
    fs::create_dir_all(path.parent().expect("note path has a folder"))?;
    let text = format!(
        "---\nproject: {name}\nrepo: {repo}\nstarted: {today}\n---\n\n# {name}\n\n{DONE_HEADING}\n",
        name = p.name,
        repo = p.repo.as_deref().unwrap_or("")
    );
    fs::write(&path, text)?;
    Ok((path, true))
}

/// The text on one line: whitespace collapsed, at most 200 characters. None when nothing is left.
pub fn one_line(text: &str) -> Option<String> {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() { None } else { Some(joined.chars().take(MAX_CHARS).collect()) }
}

/// Adds `- <date> · <text>` at the end of the note's `## Done` section. False when the text was empty.
pub fn append_done(path: &Path, date: &str, text: &str) -> Result<bool> {
    let vault = path.parent().and_then(Path::parent).unwrap_or(Path::new(""));
    need_vault(vault)?;
    let Some(text) = one_line(text) else { return Ok(false) };
    let current = if path.exists() { fs::read_to_string(path)? } else { String::new() };
    let le = if current.contains("\r\n") { "\r\n" } else { "\n" };
    let item = format!("- {date} · {text}{le}");
    let lines: Vec<&str> = current.split_inclusive('\n').collect();

    let next = match lines.iter().position(|l| l.trim() == DONE_HEADING) {
        Some(heading) => {
            let end = lines[heading + 1..].iter().position(|l| l.starts_with('#')).map_or(lines.len(), |i| heading + 1 + i);
            let last = (heading + 1..end).rev().find(|&i| !lines[i].trim().is_empty()).unwrap_or(heading);
            let mut out: String = lines[..=last].concat();
            if !out.ends_with('\n') {
                out.push_str(le);
            }
            out.push_str(&item);
            out.push_str(&lines[last + 1..].concat());
            out
        }
        None if current.is_empty() => format!("{DONE_HEADING}{le}{item}"),
        None => {
            let mut out = current.clone();
            if !out.ends_with('\n') {
                out.push_str(le);
            }
            format!("{out}{le}{DONE_HEADING}{le}{item}")
        }
    };
    fs::create_dir_all(path.parent().expect("note path has a folder"))?;
    fs::write(path, next)?;
    Ok(true)
}

fn parse_line(line: &str) -> Option<DoneLine> {
    let rest = line.trim_end_matches(['\r', '\n']).strip_prefix("- ")?;
    let (date, text) = rest.split_once(" · ")?;
    let is_date = date.len() == 10 && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    is_date.then(|| DoneLine { date: date.into(), text: text.into() })
}

/// The `## Done` lines, oldest first. A missing note has none.
pub fn read_done(path: &Path) -> Result<Vec<DoneLine>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(path)?;
    let mut in_section = false;
    let mut out = vec![];
    for line in text.lines() {
        if line.starts_with('#') {
            in_section = line.trim() == DONE_HEADING;
        } else if in_section {
            out.extend(parse_line(line));
        }
    }
    Ok(out)
}

/// `obsidian://open?path=` with the path percent-encoded as UTF-8.
pub fn obsidian_url(path: &Path) -> String {
    let mut out = String::from("obsidian://open?path=");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Each project's done lines for one day, with the project's name. Notes that can't be read are skipped.
pub fn done_on(projects: &[Project], day: &str) -> Vec<(String, DoneLine)> {
    projects
        .iter()
        .filter_map(|p| Some((p, read_done(Path::new(p.note.as_deref()?)).ok()?)))
        .flat_map(|(p, lines)| lines.into_iter().filter(|l| l.date == day).map(|l| (p.name.clone(), l)))
        .collect()
}
