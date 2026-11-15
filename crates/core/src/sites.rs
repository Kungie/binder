//! The websites on the home server: each folder in `~/sites` with a `docker-compose.yml`, its address from the
//! `hostname:` lines of its Cloudflare Tunnel config (and nothing else in that file), its containers, and what
//! they use. Watching only.

use crate::Result;
use crate::remote::Remote;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteContainer {
    pub name: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    /// The folder's name.
    pub name: String,
    /// `https://…`; the owner's saved address wins over the one found.
    pub address: Option<String>,
    pub containers: Vec<SiteContainer>,
    /// CPU percent, the site's containers added together.
    pub cpu: f32,
    pub mem_bytes: u64,
    /// The folder on disk.
    pub disk_bytes: u64,
}

/// Only `hostname:` lines leave the tunnel config, filtered on the server; the first one is the address.
const SCRIPT: &str = r#"echo --binder:sites--
cd "$HOME/sites" 2>/dev/null && for d in */; do
  d=${d%/}
  [ -f "$d/docker-compose.yml" ] || continue
  h=$(grep -hE '^[[:space:]-]*hostname:' "$d/cloudflared/config.yml" 2>/dev/null | head -n 1 | sed -E 's/^[[:space:]-]*hostname:[[:space:]]*//; s/["'"'"']//g')
  s=$(du -sb "$d" 2>/dev/null | cut -f 1)
  printf '%s|%s|%s|%s\n' "$d" "$HOME/sites/$d" "$h" "$s"
done
echo --binder:ps--
docker ps -a --format '{{.Names}}|{{.State}}|{{.Label "com.docker.compose.project.working_dir"}}' 2>/dev/null
echo --binder:stats--
docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}' 2>/dev/null
exit 0
"#;

fn section<'a>(out: &'a str, name: &str) -> &'a str {
    let marker = format!("--binder:{name}--");
    let Some(start) = out.find(&marker) else { return "" };
    let rest = &out[start + marker.len()..];
    &rest[..rest.find("--binder:").unwrap_or(rest.len())]
}

/// `38.2MiB`, `1.5GiB`, `12MB` as bytes.
pub fn bytes(s: &str) -> u64 {
    let s = s.trim();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (n, unit) = s.split_at(split);
    let n: f64 = n.parse().unwrap_or(0.0);
    let k: f64 = match unit.trim() {
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        _ => 1.0,
    };
    (n * k) as u64
}

pub fn read(remote: &dyn Remote, saved: &BTreeMap<String, String>) -> Result<Vec<Site>> {
    let out = remote.run(SCRIPT, None)?;
    let mut dirs = vec![];
    let mut sites: Vec<Site> = section(&out, "sites")
        .lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.trim().splitn(4, '|').collect();
            let [name, dir, host, size] = w.as_slice() else { return None };
            if name.is_empty() {
                return None;
            }
            dirs.push(dir.to_string());
            let found = (!host.trim().is_empty()).then(|| format!("https://{}", host.trim()));
            Some(Site {
                name: name.to_string(),
                address: saved.get(*name).cloned().or(found),
                containers: vec![],
                cpu: 0.0,
                mem_bytes: 0,
                disk_bytes: size.trim().parse().unwrap_or(0),
            })
        })
        .collect();
    let mut owner: BTreeMap<String, usize> = BTreeMap::new();
    for l in section(&out, "ps").lines() {
        let w: Vec<&str> = l.trim().splitn(3, '|').collect();
        let [name, state, dir] = w.as_slice() else { continue };
        if let Some(i) = dirs.iter().position(|d| d == dir) {
            sites[i].containers.push(SiteContainer { name: name.to_string(), state: state.to_string() });
            owner.insert(name.to_string(), i);
        }
    }
    for l in section(&out, "stats").lines() {
        let w: Vec<&str> = l.trim().splitn(3, '|').collect();
        let [name, cpu, mem] = w.as_slice() else { continue };
        let Some(&i) = owner.get(*name) else { continue };
        sites[i].cpu += cpu.trim().trim_end_matches('%').parse::<f32>().unwrap_or(0.0);
        sites[i].mem_bytes += bytes(mem.split('/').next().unwrap_or(""));
    }
    Ok(sites)
}

/// Whether a site answers, and whether it may be shown inside Binder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteCheck {
    pub status: Option<u16>,
    pub frameable: bool,
    pub error: Option<String>,
}

/// `curl -D -` headers, redirects included: the last response counts.
pub fn parse_check(headers: &str) -> SiteCheck {
    let normal = headers.replace("\r\n", "\n");
    let last = normal.split("\n\n").filter(|b| b.trim_start().starts_with("HTTP/")).last().unwrap_or("");
    let mut lines = last.trim().lines();
    let status = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse().ok());
    let mut frameable = true;
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
        if k == "x-frame-options" && (v.contains("deny") || v.contains("sameorigin")) {
            frameable = false;
        }
        if k == "content-security-policy" {
            if let Some(fa) = v.split(';').map(str::trim).find(|d| d.starts_with("frame-ancestors")) {
                if fa.trim_start_matches("frame-ancestors").trim() != "*" {
                    frameable = false;
                }
            }
        }
    }
    SiteCheck { status, frameable, error: None }
}
