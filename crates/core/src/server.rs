//! What runs on the home server: containers, services, load, memory, disk and temperature, read in one SSH call.

use crate::Result;
use crate::remote::Remote;
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
    pub at: i64,
}

/// Each read prints a marker first, and a read that fails prints nothing after it.
const SCRIPT: &str = r#"echo --binder:docker--; docker ps -a --format '{{json .}}' 2>/dev/null
echo --binder:services--; systemctl list-units --type=service --state=running --no-legend --plain 2>/dev/null
echo --binder:load--; cat /proc/loadavg 2>/dev/null
echo --binder:mem--; cat /proc/meminfo 2>/dev/null
echo --binder:disk--; df -B1 / 2>/dev/null
echo --binder:temp--; cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null
echo --binder:cores--; nproc 2>/dev/null
echo --binder:power--; vcgencmd pmic_read_adc 2>/dev/null
exit 0
"#;

pub fn fetch(remote: &dyn Remote, now: i64) -> Result<ServerStatus> {
    let out = remote.run(SCRIPT, None)?;
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
