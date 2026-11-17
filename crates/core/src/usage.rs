//! Claude Code's token use, from its transcripts (`~/.claude/projects/*/*.jsonl`) on the server and on the Mac.
//! One awk script reads the last eight days and prints a line per message; the summing happens here.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Prints `time|message id|file|input|output|cache writes|cache reads` for each line with a `usage`.
pub const SCRIPT: &str = r#"d="$HOME/.claude/projects"
[ -d "$d" ] || exit 0
find "$d" -name '*.jsonl' -mtime -8 -exec awk '
function n(k,  s) { if (match($0, "[{,]\"" k "\":[0-9]+")) { s = substr($0, RSTART, RLENGTH); sub(/.*:/, "", s); return s } return 0 }
/"usage"/ {
  ts = ""; id = ""
  if (match($0, /"timestamp":"[^"]+"/)) ts = substr($0, RSTART + 13, RLENGTH - 14)
  if (match($0, /"id":"msg_[^"]+"/)) id = substr($0, RSTART + 6, RLENGTH - 7)
  if (ts != "") print ts "|" id "|" FILENAME "|" n("input_tokens") "|" n("output_tokens") "|" n("cache_creation_input_tokens") "|" n("cache_read_input_tokens")
}' {} +
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    pub at: i64,
    pub id: String,
    pub file: String,
    /// Input, output and cache writes. Cache reads are left out: they repeat the whole context each turn.
    pub tokens: u64,
}

pub fn parse(out: &str) -> Vec<Use> {
    out.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('|').collect();
            if f.len() != 7 {
                return None;
            }
            let at = chrono::DateTime::parse_from_rfc3339(f[0]).ok()?.timestamp();
            let n = |s: &str| s.parse::<u64>().unwrap_or(0);
            Some(Use { at, id: f[1].into(), file: f[2].into(), tokens: n(f[3]) + n(f[4]) + n(f[5]) })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Usage {
    pub five_hours: u64,
    pub today: u64,
    pub week: u64,
    /// Transcripts with a message since midnight.
    pub sessions_today: usize,
}

impl std::ops::Add for Usage {
    type Output = Usage;
    fn add(self, o: Usage) -> Usage {
        Usage { five_hours: self.five_hours + o.five_hours, today: self.today + o.today, week: self.week + o.week, sessions_today: self.sessions_today + o.sessions_today }
    }
}

/// Sums the last 5 hours, since local midnight and the last 7 days. A message logged twice counts once.
pub fn summary(uses: &[Use], now: i64, midnight: i64) -> Usage {
    let mut seen = HashSet::new();
    let mut files = HashSet::new();
    let mut u = Usage::default();
    for x in uses {
        if !x.id.is_empty() && !seen.insert(x.id.as_str()) {
            continue;
        }
        if x.at > now - 7 * 86_400 {
            u.week += x.tokens;
        }
        if x.at >= midnight {
            u.today += x.tokens;
            files.insert(x.file.as_str());
        }
        if x.at > now - 5 * 3600 {
            u.five_hours += x.tokens;
        }
    }
    u.sessions_today = files.len();
    u
}

/// The number after `"key":` where the key starts a member, not inside another name.
fn num(line: &str, key: &str) -> u64 {
    let pat = format!("\"{key}\":");
    let mut from = 0;
    while let Some(i) = line[from..].find(&pat) {
        let at = from + i;
        if at > 0 && matches!(line.as_bytes()[at - 1], b'{' | b',') {
            let rest = &line[at + pat.len()..];
            let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            return rest[..end].parse().unwrap_or(0);
        }
        from = at + pat.len();
    }
    0
}

fn text_after<'a>(line: &'a str, pat: &str) -> Option<&'a str> {
    let rest = &line[line.find(pat)? + pat.len()..];
    Some(&rest[..rest.find('"')?])
}

/// What the script prints for one transcript line, read here instead.
pub fn from_line(line: &str, file: &str) -> Option<Use> {
    if !line.contains("\"usage\"") {
        return None;
    }
    let at = chrono::DateTime::parse_from_rfc3339(text_after(line, "\"timestamp\":\"")?).ok()?.timestamp();
    let id = text_after(line, "\"id\":\"msg_").map(|s| format!("msg_{s}")).unwrap_or_default();
    let tokens = num(line, "input_tokens") + num(line, "output_tokens") + num(line, "cache_creation_input_tokens");
    Some(Use { at, id, file: file.into(), tokens })
}

/// The Mac's transcripts, each file read again only when its size or time changes.
#[derive(Default)]
pub struct Cache {
    files: std::collections::HashMap<std::path::PathBuf, (std::time::SystemTime, u64, Vec<Use>)>,
}

impl Cache {
    /// Every use in `*.jsonl` files under `root` changed in the last eight days.
    pub fn read(&mut self, root: &std::path::Path) -> Vec<Use> {
        let since = std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 86_400);
        let mut found = vec![];
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for e in entries.flatten() {
                let path = e.path();
                let Ok(meta) = e.metadata() else { continue };
                if meta.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|x| x == "jsonl") && meta.modified().is_ok_and(|m| m >= since) {
                    found.push((path, meta.modified().unwrap(), meta.len()));
                }
            }
        }
        self.files.retain(|p, _| found.iter().any(|(f, _, _)| f == p));
        let mut uses = vec![];
        for (path, modified, len) in found {
            let fresh = self.files.get(&path).is_some_and(|(m, l, _)| *m == modified && *l == len);
            if !fresh {
                let name = path.to_string_lossy().into_owned();
                let list = std::fs::read_to_string(&path).map(|t| t.lines().filter_map(|l| from_line(l, &name)).collect()).unwrap_or_default();
                self.files.insert(path.clone(), (modified, len, list));
            }
            uses.extend(self.files[&path].2.iter().cloned());
        }
        uses
    }
}
