//! Claude Code's plan limits in percent, as Claude Code hands them to its status line. Binder's status line on the
//! server keeps the last input that has them in `~/.claude/binder-limits.json`, then runs any status line that was
//! there before; the Agents screen reads that file.

use crate::remote::{Remote, quote};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const COMMAND: &str = "sh ~/.claude/binder-statusline.sh";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub percent: f64,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
    /// When Claude Code last handed them over.
    pub read_at: i64,
}

/// Prints the file's time, then the file.
pub const READ_SCRIPT: &str = "f=\"$HOME/.claude/binder-limits.json\"\n[ -f \"$f\" ] || exit 0\nstat -c %Y \"$f\"\ncat \"$f\"\n";

fn time(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| chrono::DateTime::parse_from_rfc3339(v.as_str()?).ok().map(|t| t.timestamp()))
}

fn window(v: &Value, now: i64) -> Option<Window> {
    let percent = v.get("used_percentage")?.as_f64()?;
    let resets_at = v.get("resets_at").and_then(time);
    // The window started over since Claude Code last said; nothing is used in the new one yet.
    if resets_at.is_some_and(|r| r <= now) {
        return Some(Window { percent: 0.0, resets_at: None });
    }
    Some(Window { percent, resets_at })
}

pub fn parse(out: &str, now: i64) -> Option<Limits> {
    let (head, body) = out.trim().split_once('\n')?;
    let read_at = head.trim().parse().ok()?;
    let v: Value = serde_json::from_str(body.trim()).ok()?;
    let r = v.get("rate_limits")?;
    let l = Limits { five_hour: r.get("five_hour").and_then(|w| window(w, now)), seven_day: r.get("seven_day").and_then(|w| window(w, now)), read_at };
    (l.five_hour.is_some() || l.seven_day.is_some()).then_some(l)
}

/// Whether Binder's status line is the one in these settings.
pub fn installed(settings: &str) -> bool {
    serde_json::from_str::<Value>(settings).ok().is_some_and(|v| v["statusLine"]["command"] == COMMAND)
}

/// Binder's status line: keep the input when it has the limits, then hand it to the status line that was there.
fn script(previous: Option<&str>) -> String {
    let mut s = String::from(
        "#!/bin/sh\n# Binder: keeps Claude Code's plan limits for the Agents screen.\ni=$(cat)\nf=\"$HOME/.claude/binder-limits.json\"\ncase $i in *'\"rate_limits\"'*) printf '%s\\n' \"$i\" > \"$f.tmp\" && mv \"$f.tmp\" \"$f\" ;; esac\n",
    );
    if let Some(p) = previous {
        s.push_str(&format!("printf '%s' \"$i\" | sh -c {}\n", quote(p)));
    }
    s
}

/// Adds Binder's status line to the server's Claude Code settings, once, keeping a copy of the settings first.
pub fn install(remote: &dyn Remote) -> Result<()> {
    let current = remote.run("cat \"$HOME/.claude/settings.json\" 2>/dev/null || true\n", None)?;
    if installed(&current) {
        return Ok(());
    }
    let mut v: Value = if current.trim().is_empty() { Value::Object(Default::default()) } else { serde_json::from_str(&current).map_err(|e| Error::Invalid(format!("Couldn't read Claude Code's settings: {e}")))? };
    let previous = v["statusLine"]["command"].as_str().map(String::from);
    remote.run("mkdir -p \"$HOME/.claude\"\ncat > \"$HOME/.claude/binder-statusline.sh\"\n", Some(&script(previous.as_deref())))?;
    let obj = v.as_object_mut().ok_or_else(|| Error::Invalid("Claude Code's settings aren't an object.".into()))?;
    obj.insert("statusLine".into(), serde_json::json!({ "type": "command", "command": COMMAND }));
    let text = serde_json::to_string_pretty(&v).map_err(|e| Error::Invalid(e.to_string()))? + "\n";
    remote.run(
        "s=\"$HOME/.claude/settings.json\"\n[ -f \"$s\" ] && [ ! -f \"$s.binder-backup\" ] && cp \"$s\" \"$s.binder-backup\"\ncat > \"$s.tmp\" && mv \"$s.tmp\" \"$s\"\n",
        Some(&text),
    )?;
    Ok(())
}
