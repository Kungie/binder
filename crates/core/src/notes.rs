//! Project notes in the Obsidian vault. Each project has a folder, `Projects/<name>/`, holding its note
//! (written once from a template) and `Daily/<date>.md` files that Binder only appends done lines to. Every
//! other byte belongs to the owner. Notes from before 2.2 sit at `Projects/<name>.md` and keep `## Done`
//! lines; they're moved into the folder and still read.

use crate::model::Project;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneLine {
    pub date: String,
    /// `HH:MM`, local. Lines from the old `## Done` section have none.
    #[serde(default)]
    pub time: Option<String>,
    pub text: String,
}

/// A done line with the project it belongs to, for the overview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoneItem {
    pub project_id: i64,
    pub project: String,
    pub color: String,
    pub line: DoneLine,
}

const DONE_HEADING: &str = "## Done";
const MAX_CHARS: usize = 200;

/// `vault/Projects/<name>/<name>.md`, with characters a file name or Obsidian link can't hold replaced by `-`.
pub fn note_path(vault: &Path, project_name: &str) -> PathBuf {
    let stem = note_stem(project_name);
    vault.join("Projects").join(&stem).join(format!("{stem}.md"))
}

/// Where notes lived before 2.2: `vault/Projects/<name>.md`.
pub fn legacy_note_path(vault: &Path, project_name: &str) -> PathBuf {
    vault.join("Projects").join(format!("{}.md", note_stem(project_name)))
}

/// A note that sits loose in `Projects/` moved into its own folder, when that spot is free. Its bytes don't change.
pub fn move_into_folder(note: &Path) -> Result<PathBuf> {
    let (Some(dir), Some(stem)) = (note.parent(), note.file_stem()) else { return Ok(note.to_path_buf()) };
    if dir.file_name().is_none_or(|d| d != "Projects") || !note.is_file() {
        return Ok(note.to_path_buf());
    }
    let target = dir.join(stem).join(note.file_name().expect("has a stem"));
    if target.exists() {
        return Ok(note.to_path_buf());
    }
    fs::create_dir_all(target.parent().expect("has a folder"))?;
    fs::rename(note, &target)?;
    Ok(target)
}

/// The note's file name without `.md`. Two projects with the same stem (ignoring case) would share a note.
pub fn note_stem(project_name: &str) -> String {
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
    if safe.is_empty() { "Untitled".into() } else { safe.into() }
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
    let legacy = legacy_note_path(vault, &p.name);
    if legacy.is_file() {
        return Ok((move_into_folder(&legacy)?, false));
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

/// `<note folder>/Daily/<date>.md`.
pub fn daily_path(note: &Path, date: &str) -> PathBuf {
    note.parent().unwrap_or(Path::new("")).join("Daily").join(format!("{date}.md"))
}

fn daily_header(project_name: &str, date: &str) -> String {
    let title = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|d| d.format("%-d %B %Y").to_string()).unwrap_or_else(|_| date.into());
    format!("---\nproject: \"[[{}]]\"\ndate: {date}\n---\n\n# {project_name}, {title}\n\n", note_stem(project_name))
}

/// Adds `- HH:MM · <text>` (or `- <text>` without a time) to the end of the day's file, making it with its header
/// first. Lines already there are never touched. False when the text was empty.
pub fn append_daily(note: &Path, project_name: &str, date: &str, time: Option<&str>, text: &str) -> Result<bool> {
    let vault = note.ancestors().nth(3).unwrap_or(Path::new(""));
    need_vault(vault)?;
    let Some(text) = one_line(text) else { return Ok(false) };
    let path = daily_path(note, date);
    fs::create_dir_all(path.parent().expect("daily path has a folder"))?;
    let mut out = if path.exists() {
        let current = fs::read_to_string(&path)?;
        if current.is_empty() || current.ends_with('\n') { String::new() } else { "\n".into() }
    } else {
        daily_header(project_name, date)
    };
    match time {
        Some(t) => out.push_str(&format!("- {t} · {text}\n")),
        None => out.push_str(&format!("- {text}\n")),
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(&path)?;
    std::io::Write::write_all(&mut f, out.as_bytes())?;
    Ok(true)
}

fn is_time(s: &str) -> bool {
    s.len() == 5 && s.chars().enumerate().all(|(i, c)| if i == 2 { c == ':' } else { c.is_ascii_digit() })
}

fn is_date(s: &str) -> bool {
    s.len() == 10 && s.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
}

/// The day's lines, oldest first. Lines in the frontmatter are skipped; a missing file has none.
pub fn read_daily(note: &Path, date: &str) -> Result<Vec<DoneLine>> {
    let path = daily_path(note, date);
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines().peekable();
    if lines.peek().is_some_and(|l| l.trim_end() == "---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim_end() == "---" {
                break;
            }
        }
    }
    Ok(lines
        .filter_map(|l| l.trim_end().strip_prefix("- "))
        .filter(|rest| !rest.trim().is_empty())
        .map(|rest| match rest.split_once(" · ") {
            Some((t, text)) if is_time(t) => DoneLine { date: date.into(), time: Some(t.into()), text: text.into() },
            _ => DoneLine { date: date.into(), time: None, text: rest.into() },
        })
        .collect())
}

/// Every done line of a project, newest first: the daily files newest day first, then the old `## Done` lines.
pub fn read_history(note: &Path) -> Result<Vec<DoneLine>> {
    let dir = note.parent().unwrap_or(Path::new("")).join("Daily");
    let mut days: Vec<String> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".md").map(String::from))
            .filter(|d| is_date(d))
            .collect(),
        Err(_) => vec![],
    };
    days.sort_unstable_by(|a, b| b.cmp(a));
    let mut out = vec![];
    for day in days {
        out.extend(read_daily(note, &day)?.into_iter().rev());
    }
    out.extend(read_done(note)?.into_iter().rev());
    Ok(out)
}

fn parse_line(line: &str) -> Option<DoneLine> {
    let rest = line.trim_end_matches(['\r', '\n']).strip_prefix("- ")?;
    let (date, text) = rest.split_once(" · ")?;
    is_date(date).then(|| DoneLine { date: date.into(), time: None, text: text.into() })
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

/// Each project's done lines for one day, newest first; lines without a time go last. Notes that can't be read are
/// skipped.
pub fn done_on(projects: &[Project], day: &str) -> Vec<DoneItem> {
    let mut out: Vec<DoneItem> = projects
        .iter()
        .filter_map(|p| {
            let note = Path::new(p.note.as_deref()?);
            let mut lines = read_daily(note, day).ok()?;
            lines.extend(read_done(note).ok()?.into_iter().filter(|l| l.date == day));
            Some((p, lines))
        })
        .flat_map(|(p, lines)| {
            lines.into_iter().map(|line| DoneItem { project_id: p.id, project: p.name.clone(), color: p.color.clone(), line })
        })
        .collect();
    // Within one time, the later line in the file is newer.
    out.reverse();
    out.sort_by(|a, b| b.line.time.cmp(&a.line.time));
    out
}
