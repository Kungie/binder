//! The websites on the home server: each folder in `~/sites` with a compose file, and every compose project with a
//! Cloudflare Tunnel wherever it lives, so a site hosted anywhere shows up by itself. The address comes from the
//! `hostname:` lines of the tunnel config (and nothing else in that file), or from the tunnel's log when the tunnel
//! is set up on Cloudflare's side. Then its containers, and what they use. Watching only.

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

/// Only `hostname:` lines leave the tunnel config, filtered on the server; the first one is the address. A tunnel set
/// up on Cloudflare's side has no config here: its log's last configuration line names the hostnames, and only the
/// first hostname leaves. Each tunnel's compose project is a site too, wherever its folder is.
const SCRIPT: &str = r#"echo --binder:sites--
cd "$HOME/sites" 2>/dev/null && for d in */; do
  d=${d%/}
  [ -f "$d/docker-compose.yml" ] || [ -f "$d/compose.yml" ] || [ -f "$d/compose.yaml" ] || [ -f "$d/docker-compose.yaml" ] || continue
  h=$(grep -hE '^[[:space:]-]*hostname:' "$d/cloudflared/config.yml" 2>/dev/null | head -n 1 | sed -E 's/^[[:space:]-]*hostname:[[:space:]]*//; s/["'"'"']//g')
  s=$(du -sb "$d" 2>/dev/null | cut -f 1)
  printf '%s|%s|%s|%s\n' "$d" "$HOME/sites/$d" "$h" "$s"
done
echo --binder:ps--
docker ps -a --format '{{.Names}}|{{.State}}|{{.Label "com.docker.compose.project.working_dir"}}|{{.Label "com.docker.compose.project"}}|{{.Image}}' 2>/dev/null
echo --binder:tunnels--
docker ps -a --format '{{.Names}}|{{.Image}}|{{.Label "com.docker.compose.project"}}|{{.Label "com.docker.compose.project.working_dir"}}' 2>/dev/null | while IFS='|' read -r n i p w; do
  case "$i" in *cloudflared*) ;; *) continue ;; esac
  [ -n "$p" ] || continue
  h=$(docker logs "$n" </dev/null 2>&1 | grep 'hostname' | tail -n 1 | grep -oE 'hostname\\?":\\?"[A-Za-z0-9.-]+' | head -n 1 | sed -E 's/.*"//')
  o=$(docker ps -a --filter "label=com.docker.compose.project=$p" --format '{{.Image}}|{{.Label "com.docker.compose.project.working_dir"}}' </dev/null 2>/dev/null | grep -v '^[^|]*cloudflared' | head -n 1 | cut -d '|' -f 2)
  [ -n "$o" ] && w=$o
  s=$(du -sb "$w" 2>/dev/null | cut -f 1)
  printf '%s|%s|%s|%s\n' "$p" "$w" "$h" "$s"
done
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
    let site = |name: &str, found: Option<String>, size: &str| Site {
        name: name.to_string(),
        address: saved.get(name).cloned().or(found),
        containers: vec![],
        cpu: 0.0,
        mem_bytes: 0,
        disk_bytes: size.trim().parse().unwrap_or(0),
    };
    let https = |h: &str| (!h.trim().is_empty()).then(|| format!("https://{}", h.trim()));
    let mut dirs: Vec<String> = vec![];
    let mut sites: Vec<Site> = vec![];
    for l in section(&out, "sites").lines() {
        let w: Vec<&str> = l.trim().splitn(4, '|').collect();
        let [name, dir, host, size] = w.as_slice() else { continue };
        if name.is_empty() {
            continue;
        }
        dirs.push(dir.to_string());
        sites.push(site(name, https(host), size));
    }
    // name, state, folder, compose project
    let ps: Vec<[String; 4]> = section(&out, "ps")
        .lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.trim().split('|').collect();
            (w.len() >= 3 && !w[0].is_empty()).then(|| [w[0], w[1], w[2], w.get(3).copied().unwrap_or("")].map(String::from))
        })
        .collect();
    // A compose project belongs to the site whose folder one of its containers runs from.
    let mut projects: BTreeMap<String, usize> = BTreeMap::new();
    for [_, _, dir, project] in &ps {
        if let Some(i) = dirs.iter().position(|d| d == dir) {
            if !project.is_empty() {
                projects.entry(project.clone()).or_insert(i);
            }
        }
    }
    for l in section(&out, "tunnels").lines() {
        let w: Vec<&str> = l.trim().splitn(4, '|').collect();
        let [project, dir, host, size] = w.as_slice() else { continue };
        if project.is_empty() {
            continue;
        }
        let known = projects.get(*project).copied().or_else(|| dirs.iter().position(|d| d == dir));
        match known {
            Some(i) => {
                if sites[i].address.is_none() {
                    sites[i].address = https(host);
                }
            }
            None => {
                projects.insert(project.to_string(), sites.len());
                dirs.push(dir.to_string());
                sites.push(site(project, https(host), size));
            }
        }
    }
    let mut owner: BTreeMap<String, usize> = BTreeMap::new();
    for [name, state, dir, project] in &ps {
        let Some(i) = dirs.iter().position(|d| d == dir).or_else(|| projects.get(project).copied()) else { continue };
        sites[i].containers.push(SiteContainer { name: name.clone(), state: state.clone() });
        owner.insert(name.clone(), i);
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

/// A page fetched for a site that refuses to be framed, made safe to show as a still picture: every relative
/// address resolves against the site, and scripts and refreshes are taken out (the frame runs none anyway).
pub fn snapshot(html: &str, url: &str) -> String {
    let mut doc = html.to_string();
    // Scripts, whole: `<script …>` to the end of its `</script>`.
    loop {
        let low = doc.to_ascii_lowercase();
        let Some(start) = low.find("<script") else { break };
        let end = low[start..].find("</script").map(|e| start + e).and_then(|e| low[e..].find('>').map(|g| e + g + 1)).unwrap_or(doc.len());
        doc.replace_range(start..end, "");
    }
    // Refreshes would carry the frame elsewhere.
    let mut from = 0;
    loop {
        let low = doc.to_ascii_lowercase();
        let Some(start) = low[from..].find("<meta").map(|s| from + s) else { break };
        let end = low[start..].find('>').map(|g| start + g + 1).unwrap_or(doc.len());
        let tag = &low[start..end];
        if tag.contains("http-equiv") && tag.contains("refresh") {
            doc.replace_range(start..end, "");
            from = start;
        } else {
            from = end;
        }
    }
    let href = url.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;");
    // Nothing moves in a preview: a still page costs nothing once drawn.
    let base = format!("<base href=\"{href}\"><style>*, *::before, *::after {{ animation: none !important; transition: none !important; scroll-behavior: auto !important; }}</style>");
    let low = doc.to_ascii_lowercase();
    let head = low.match_indices("<head").map(|(i, _)| i).find(|i| matches!(low.as_bytes().get(i + 5), Some(b'>' | b' ' | b'\t' | b'\n' | b'\r')));
    match head.and_then(|h| low[h..].find('>').map(|g| h + g + 1)) {
        Some(at) => doc.insert_str(at, &base),
        None => doc.insert_str(0, &base),
    }
    doc
}
