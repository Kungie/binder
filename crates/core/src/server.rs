//! What runs on the home server: containers, services, load, memory, disk and temperature, read in one SSH call.

use crate::Result;
use crate::remote::{Remote, quote};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerStatus {
    pub containers: Vec<Container>,
    pub services: Vec<Service>,
    pub load: [f32; 3],
    pub mem_total: u64,
    pub mem_available: u64,
    pub disk_size: u64,
    pub disk_used: u64,
    pub temp_c: Option<f32>,
    /// From `nproc`; 0 when unknown.
    pub cores: u32,
    /// The Pi 5's board draw in watts, from its PMIC; `None` on other servers.
    pub power_w: Option<f32>,
    /// The SD card inside Cloud, when it is a disk of its own.
    pub sd_size: Option<u64>,
    pub sd_used: Option<u64>,
    /// `docker ps` failed (the daemon restarting, say): the empty list says nothing about the containers.
    #[serde(default)]
    pub docker_failed: bool,
    /// When the server booted, from its uptime; `None` when it couldn't be read.
    #[serde(default)]
    pub boot_at: Option<i64>,
    pub at: i64,
}

/// Each read prints a marker first, and a read that fails prints nothing after it.
const SCRIPT: &str = r#"echo --binder:docker--; docker ps -a --format '{{json .}}' 2>/dev/null || echo --binder:docker-failed--
echo --binder:services--; systemctl list-units --type=service --state=running --no-legend --plain 2>/dev/null
echo --binder:load--; cat /proc/loadavg 2>/dev/null
echo --binder:mem--; cat /proc/meminfo 2>/dev/null
echo --binder:disk--; df -B1 / 2>/dev/null
echo --binder:temp--; cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null
echo --binder:cores--; nproc 2>/dev/null
echo --binder:power--; vcgencmd pmic_read_adc 2>/dev/null
echo --binder:uptime--; cat /proc/uptime 2>/dev/null
exit 0
"#;

/// One call for everything on the Server screen. `sd` is the SD card's mount point inside Cloud, if any.
pub fn fetch(remote: &dyn Remote, now: i64, sd: Option<&str>) -> Result<ServerStatus> {
    let script = match sd {
        Some(sd) => SCRIPT.replace("exit 0\n", &format!("echo --binder:sd--; df -B1 --output=target,size,used {} 2>/dev/null\nexit 0\n", quote(sd))),
        None => SCRIPT.to_string(),
    };
    let out = remote.run(&script, None)?;
    let part = |name: &str| section(&out, name);
    let (mem_total, mem_available) = parse_meminfo(part("mem"));
    let (disk_size, disk_used) = parse_df(part("disk"));
    Ok(ServerStatus {
        containers: parse_docker(part("docker")),
        services: parse_services(part("services")),
        load: parse_loadavg(part("load")),
        mem_total,
        mem_available,
        disk_size,
        disk_used,
        temp_c: parse_temp(part("temp")),
        cores: parse_cores(part("cores")),
        power_w: parse_power(part("power")),
        sd_size: sd.and_then(|sd| parse_mount(part("sd"), sd)).map(|(s, _)| s),
        sd_used: sd.and_then(|sd| parse_mount(part("sd"), sd)).map(|(_, u)| u),
        docker_failed: out.contains("--binder:docker-failed--"),
        boot_at: part("uptime").split_whitespace().next().and_then(|u| u.parse::<f64>().ok()).map(|u| now - u.round() as i64),
        at: now,
    })
}

fn section<'a>(out: &'a str, name: &str) -> &'a str {
    let marker = format!("--binder:{name}--");
    let Some(start) = out.find(&marker) else { return "" };
    let rest = &out[start + marker.len()..];
    &rest[..rest.find("--binder:").unwrap_or(rest.len())]
}

pub fn parse_docker(s: &str) -> Vec<Container> {
    s.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .map(|v| {
            let f = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
            Container { name: f("Names"), image: f("Image"), state: f("State"), status: f("Status") }
        })
        .collect()
}

pub fn parse_services(s: &str) -> Vec<Service> {
    s.lines()
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let unit = words.next()?;
            let name = unit.strip_suffix(".service")?;
            let description = words.skip(3).collect::<Vec<_>>().join(" ");
            Some(Service { name: name.to_string(), description })
        })
        .collect()
}

pub fn parse_loadavg(s: &str) -> [f32; 3] {
    let mut v = s.split_whitespace().map(|w| w.parse::<f32>().unwrap_or(0.0));
    [v.next().unwrap_or(0.0), v.next().unwrap_or(0.0), v.next().unwrap_or(0.0)]
}

/// (total, available) in bytes.
pub fn parse_meminfo(s: &str) -> (u64, u64) {
    let kb = |key: &str| {
        s.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|rest| rest.trim_start_matches(':').split_whitespace().next()?.parse::<u64>().ok())
            .map_or(0, |v| v * 1024)
    };
    (kb("MemTotal"), kb("MemAvailable"))
}

/// (size, used) in bytes for the root filesystem.
pub fn parse_df(s: &str) -> (u64, u64) {
    s.lines()
        .find_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            Some((w.get(1)?.parse().ok()?, w.get(2)?.parse().ok()?))
        })
        .unwrap_or((0, 0))
}

/// The core count `nproc` printed; 0 when it's missing or not a number.
pub fn parse_cores(s: &str) -> u32 {
    s.trim().parse().unwrap_or(0)
}

pub fn parse_temp(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().map(|m| m / 1000.0)
}

/// Sums volts times amps over the rails `vcgencmd pmic_read_adc` reports with both a current and a voltage line
/// (`3V3_SYS_A current(1)=0.13A`, `3V3_SYS_V volt(9)=3.30V`). Rails with only one side are skipped.
pub fn parse_power(s: &str) -> Option<f32> {
    let mut rails: std::collections::HashMap<&str, (Option<f32>, Option<f32>)> = std::collections::HashMap::new();
    for line in s.lines() {
        let Some((name, value)) = line.trim().split_once(' ') else { continue };
        let Some((rail, side)) = name.rsplit_once('_') else { continue };
        let Some((_, number)) = value.split_once('=') else { continue };
        let unit = if side == "A" { 'A' } else if side == "V" { 'V' } else { continue };
        let Some(Ok(x)) = number.strip_suffix(unit).map(str::parse::<f32>) else { continue };
        let entry = rails.entry(rail).or_default();
        if side == "A" { entry.0 = Some(x) } else { entry.1 = Some(x) }
    }
    let pairs: Vec<f32> = rails.values().filter_map(|(a, v)| Some((*a)? * (*v)?)).collect();
    (!pairs.is_empty()).then(|| pairs.iter().sum())
}

/// (size, used) from `df --output=target,size,used`, only when the mount point is `path` itself: a folder
/// that isn't a disk of its own would count the disk it sits on twice.
pub fn parse_mount(s: &str, path: &str) -> Option<(u64, u64)> {
    let line = s.lines().filter(|l| !l.trim().is_empty()).nth(1)?;
    let w: Vec<&str> = line.split_whitespace().collect();
    let [.., size, used] = w.as_slice() else { return None };
    let (size, used) = (size.parse().ok()?, used.parse().ok()?);
    let target = w[..w.len() - 2].join(" ");
    (target == path.trim_end_matches('/')).then_some((size, used))
}

/// What Docker remembers about a container's last start and stop, for the time the Mac wasn't watching.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerTimes {
    pub name: String,
    pub state: String,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub restarts: u32,
}

/// Every container's start and stop times in one call. Names come from `docker ps` on the server, never from Binder.
pub fn container_times(remote: &dyn Remote) -> Result<Vec<ContainerTimes>> {
    let script = "echo --binder:inspect--\nids=$(docker ps -aq 2>/dev/null)\n[ -n \"$ids\" ] && docker inspect --format '{{.Name}}|{{.State.Status}}|{{.State.StartedAt}}|{{.State.FinishedAt}}|{{.RestartCount}}' $(docker ps -aq) 2>/dev/null\nexit 0\n";
    let out = remote.run(script, None)?;
    Ok(section(&out, "inspect").lines().filter_map(parse_times).collect())
}

/// Docker writes `0001-01-01T00:00:00Z` for a time that never happened.
fn docker_time(s: &str) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(s.trim()).ok()?.timestamp();
    (t > 0).then_some(t)
}

fn parse_times(line: &str) -> Option<ContainerTimes> {
    let w: Vec<&str> = line.trim().split('|').collect();
    let [name, state, started, finished, restarts] = w.as_slice() else { return None };
    Some(ContainerTimes {
        name: name.trim_start_matches('/').to_string(),
        state: state.to_string(),
        started_at: docker_time(started),
        finished_at: docker_time(finished),
        restarts: restarts.trim().parse().unwrap_or(0),
    })
}

/// A container's last 200 log lines, both streams, oldest first.
pub fn logs(remote: &dyn Remote, name: &str) -> Result<String> {
    let out = remote.run(&format!("echo --binder:logs--\ndocker logs --tail 200 {} 2>&1\nexit 0\n", quote(name)), None)?;
    Ok(section(&out, "logs").trim_matches('\n').to_string())
}

pub fn restart(remote: &dyn Remote, name: &str) -> Result<()> {
    remote.run(&format!("docker restart {} >/dev/null\n", quote(name)), None)?;
    Ok(())
}

/// The owner's restart: logged first, so the watch knows the stop and start that follow are theirs. The database is
/// free again before the SSH call.
pub fn restart_and_log(db: &std::sync::Mutex<crate::Db>, remote: &dyn Remote, name: &str, now: i64) -> Result<()> {
    db.lock().unwrap_or_else(|e| e.into_inner()).log_event(now, crate::events::EventKind::OwnerRestart, name, "")?;
    restart(remote, name)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopProcess {
    pub name: String,
    pub cpu: f32,
    /// Share of memory, in percent.
    pub mem: f32,
    pub rss_mb: u64,
}

/// The 10 processes using the most CPU, or the most memory.
pub fn processes(remote: &dyn Remote, by_memory: bool) -> Result<Vec<TopProcess>> {
    let sort = if by_memory { "-rss" } else { "-pcpu" };
    let out = remote.run(&format!("echo --binder:ps--\nps -eo comm=,pcpu=,pmem=,rss= --sort={sort} 2>/dev/null | head -n 10\nexit 0\n"), None)?;
    Ok(section(&out, "ps")
        .lines()
        .filter_map(|l| {
            // The name may hold spaces; the last three words are the numbers.
            let w: Vec<&str> = l.split_whitespace().collect();
            let [.., cpu, mem, rss] = w.as_slice() else { return None };
            let (cpu, mem, rss): (f32, f32, u64) = (cpu.parse().ok()?, mem.parse().ok()?, rss.parse().ok()?);
            let name = w[..w.len() - 3].join(" ");
            if name.is_empty() {
                return None;
            }
            Some(TopProcess { name, cpu, mem, rss_mb: rss / 1024 })
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopFolder {
    pub path: String,
    pub bytes: u64,
}

/// The 10 largest folders two levels down each root, each disk counted once (`du -x`).
pub fn folders(remote: &dyn Remote, roots: &[&str]) -> Result<Vec<TopFolder>> {
    let du: Vec<String> = roots.iter().map(|r| format!("timeout 60 du -x -B1 -d2 {} 2>/dev/null", quote(r))).collect();
    let out = remote.run(&format!("echo --binder:du--\n{}\nexit 0\n", du.join("\n")), None)?;
    Ok(parse_folders(section(&out, "du"), roots))
}

/// `du` lines as the largest folders: the roots themselves go, and so does a folder whose sub-folder on the
/// same disk is also listed, so each one says where the space really is.
pub fn parse_folders(out: &str, roots: &[&str]) -> Vec<TopFolder> {
    let root_of = |p: &str| -> usize {
        roots
            .iter()
            .enumerate()
            .filter(|(_, r)| **r == "/" || p == **r || p.starts_with(&format!("{r}/")))
            .max_by_key(|(_, r)| r.len())
            .map_or(usize::MAX, |(i, _)| i)
    };
    let all: Vec<TopFolder> = out
        .lines()
        .filter_map(|l| {
            let (bytes, path) = l.split_once('\t')?;
            Some(TopFolder { path: path.trim().to_string(), bytes: bytes.trim().parse().ok()? })
        })
        .filter(|f| !roots.contains(&f.path.as_str()))
        .collect();
    let mut keep: Vec<TopFolder> = all
        .iter()
        .filter(|f| {
            let prefix = format!("{}/", f.path.trim_end_matches('/'));
            !all.iter().any(|g| g.path.starts_with(&prefix) && root_of(&g.path) == root_of(&f.path))
        })
        .cloned()
        .collect();
    keep.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    keep.truncate(10);
    keep
}
