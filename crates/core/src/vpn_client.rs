//! Binder VPN, the friends' menu bar app: its sign-in answer, Tailscale's profiles and the exit nodes as regions.
//! The app runs the commands; this is the part that can be tested.

use crate::tailscale::Status;
use serde::{Deserialize, Serialize};

/// The sign-in service, behind the Binder website's tunnel.
pub const SERVICE: &str = "https://binder.erenkahraman.dev/vpn/api/login";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub tailnet: String,
    pub account: String,
    pub current: bool,
}

/// `tailscale switch --list`: a header, then one profile a line, the current one's account marked with `*`.
pub fn parse_profiles(out: &str) -> Vec<Profile> {
    out.lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return None;
            }
            let account = parts[parts.len() - 1];
            Some(Profile {
                id: parts[0].to_string(),
                tailnet: parts[1..parts.len() - 1].join(" "),
                account: account.trim_end_matches('*').to_string(),
                current: account.ends_with('*'),
            })
        })
        .collect()
}

pub fn current_profile(out: &str) -> Option<String> {
    parse_profiles(out).into_iter().find(|p| p.current).map(|p| p.id)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Login {
    pub key: String,
    pub hostname: String,
}

pub fn login_body(user: &str, password: &str) -> String {
    serde_json::json!({ "user": user.trim().to_lowercase(), "password": password }).to_string()
}

fn valid_host(h: &str) -> bool {
    (1..=63).contains(&h.len()) && h.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !h.starts_with('-')
}

/// The service's answer: the key and the host name, or its own words for what went wrong.
pub fn parse_login(body: &str, code: u16) -> Result<Login, String> {
    #[derive(Deserialize)]
    struct Raw {
        key: Option<String>,
        hostname: Option<String>,
        error: Option<String>,
    }
    let down = || "Binder VPN's service didn't answer. Try again later.".to_string();
    let raw: Raw = serde_json::from_str(body).map_err(|_| down())?;
    if code != 200 {
        return Err(raw.error.unwrap_or_else(down));
    }
    match (raw.key, raw.hostname) {
        (Some(key), Some(hostname)) if valid_host(&hostname) && !key.is_empty() && key.bytes().all(|b| b.is_ascii_graphic()) => Ok(Login { key, hostname }),
        _ => Err(down()),
    }
}

/// Everything set again from nothing, so `up` never refuses over a setting changed elsewhere.
pub fn up_args(host: &str, exit: &str) -> Vec<String> {
    vec!["up".into(), "--reset".into(), format!("--hostname={host}"), format!("--exit-node={exit}"), "--exit-node-allow-lan-access".into()]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub ip: String,
    pub city: String,
    pub online: bool,
    pub active: bool,
}

/// Tailscale's DERP region codes, as cities. The same places as the Binder app's VPN screen.
pub fn city(code: &str) -> Option<&'static str> {
    Some(match code {
        "ams" => "Amsterdam",
        "fra" => "Frankfurt",
        "nue" => "Nuremberg",
        "lhr" => "London",
        "par" => "Paris",
        "mad" => "Madrid",
        "waw" => "Warsaw",
        "hel" => "Helsinki",
        "sto" => "Stockholm",
        "vie" => "Vienna",
        "zrh" => "Zurich",
        "ist" => "Istanbul",
        "dbi" => "Dubai",
        "jnb" => "Johannesburg",
        "nai" => "Nairobi",
        "blr" => "Bangalore",
        "sin" => "Singapore",
        "hkg" => "Hong Kong",
        "tok" => "Tokyo",
        "syd" => "Sydney",
        "hnl" => "Honolulu",
        "sea" => "Seattle",
        "sfo" => "San Francisco",
        "lax" => "Los Angeles",
        "den" => "Denver",
        "dfw" => "Dallas",
        "ord" => "Chicago",
        "tor" => "Toronto",
        "nyc" => "New York",
        "iad" => "Ashburn",
        "mia" => "Miami",
        "sao" => "São Paulo",
        _ => return None,
    })
}

/// Every exit node the tailnet offers, by name, its city from its relay.
pub fn regions(status: &Status) -> Vec<Region> {
    status
        .peers
        .iter()
        .filter(|p| p.exit_option && !p.this_mac)
        .map(|p| Region {
            name: p.name.clone(),
            ip: p.ip.clone(),
            city: city(&p.relay).map(String::from).unwrap_or_else(|| p.name.clone()),
            online: p.online,
            active: p.exit_node,
        })
        .collect()
}
