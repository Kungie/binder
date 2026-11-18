//! The tailnet, as the Mac's Tailscale app reports it (`tailscale status --json`, `tailscale ping`). No login or API
//! key: the app already knows every device.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    pub name: String,
    /// The MagicDNS name, without the trailing dot.
    pub dns: String,
    pub os: String,
    /// The first Tailscale address, IPv4 when there is one.
    pub ip: String,
    pub online: bool,
    /// When it was last seen, for one that is offline.
    pub last_seen: Option<i64>,
    pub key_expiry: Option<i64>,
    pub expired: bool,
    /// The DERP region traffic goes through when there is no direct path.
    pub relay: String,
    /// Whether files can be sent to it with Taildrop.
    pub taildrop: bool,
    pub this_mac: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Status {
    pub running: bool,
    /// This Mac first, then the others by name.
    pub peers: Vec<Peer>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct RawPeer {
    #[serde(rename = "ID")]
    id: String,
    host_name: String,
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "OS")]
    os: String,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
    online: bool,
    last_seen: Option<String>,
    key_expiry: Option<String>,
    expired: bool,
    relay: String,
    taildrop_target: i64,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "PascalCase")]
struct RawStatus {
    backend_state: String,
    #[serde(rename = "Self")]
    me: Option<RawPeer>,
    peer: Option<HashMap<String, RawPeer>>,
}

/// A time Tailscale gives, or none for its zero time.
fn time(s: &Option<String>) -> Option<i64> {
    let t = chrono::DateTime::parse_from_rfc3339(s.as_deref()?).ok()?.timestamp();
    (t > 0).then_some(t)
}

fn peer(r: RawPeer, this_mac: bool) -> Peer {
    let ips = r.tailscale_ips.unwrap_or_default();
    let ip = ips.iter().find(|a| a.contains('.')).or(ips.first()).cloned().unwrap_or_default();
    Peer {
        id: r.id,
        name: r.host_name,
        dns: r.dns_name.trim_end_matches('.').to_string(),
        os: r.os,
        ip,
        online: r.online,
        last_seen: time(&r.last_seen),
        key_expiry: time(&r.key_expiry),
        expired: r.expired,
        relay: r.relay,
        // 5 and 9 are the values Tailscale reports for a device that accepts files.
        taildrop: !this_mac && r.taildrop_target != 0,
        this_mac,
    }
}

pub fn parse_status(json: &str) -> Result<Status> {
    let raw: RawStatus = serde_json::from_str(json).map_err(|e| Error::Invalid(format!("Couldn't read Tailscale's status: {e}")))?;
    let mut peers: Vec<Peer> = raw.peer.unwrap_or_default().into_values().map(|p| peer(p, false)).collect();
    peers.sort_by_key(|p| p.name.to_lowercase());
    if let Some(me) = raw.me {
        peers.insert(0, peer(me, true));
    }
    Ok(Status { running: raw.backend_state == "Running", peers })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ping {
    pub ms: f64,
    /// "direct", or "DERP <region>" through a relay.
    pub via: String,
}

/// The last `pong` line of `tailscale ping`.
pub fn parse_ping(out: &str) -> Option<Ping> {
    let line = out.lines().rev().find(|l| l.starts_with("pong from "))?;
    let (_, rest) = line.split_once(" via ")?;
    let (via, time) = rest.rsplit_once(" in ")?;
    let ms = time.trim().strip_suffix("ms")?.parse().ok()?;
    let via = match via.strip_prefix("DERP(").and_then(|v| v.strip_suffix(')')) {
        Some(region) => format!("DERP {region}"),
        None => "direct".into(),
    };
    Some(Ping { ms, via })
}

/// Phones and tablets sleep and wake all day; only computers' comings and goings are worth a notification.
fn computer(p: &Peer) -> bool {
    !matches!(p.os.as_str(), "iOS" | "android" | "tvOS")
}

/// Computers that went offline (false) or came back (true) since the last call. `seen` keeps each one's last state.
pub fn changes(seen: &mut HashMap<String, bool>, now: &[Peer]) -> Vec<(String, bool)> {
    let mut out = vec![];
    for p in now.iter().filter(|p| !p.this_mac && computer(p)) {
        if let Some(before) = seen.insert(p.id.clone(), p.online)
            && before != p.online
        {
            out.push((p.name.clone(), p.online));
        }
    }
    out
}

/// Devices whose key has expired (0 days) or expires within a week, with the days left.
pub fn expiring(peers: &[Peer], now: i64) -> Vec<(String, i64)> {
    peers
        .iter()
        .filter_map(|p| {
            let left = if p.expired { 0 } else { (p.key_expiry? - now).max(0) / 86_400 };
            (p.expired || p.key_expiry? - now < 7 * 86_400).then(|| (p.name.clone(), left))
        })
        .collect()
}

/// A host for `ssh`: letters, digits and `._@:-`, not starting with a dash.
pub fn valid_target(t: &str) -> bool {
    !t.is_empty() && t.len() <= 200 && !t.starts_with('-') && t.chars().all(|c| c.is_ascii_alphanumeric() || "._@:-".contains(c))
}
