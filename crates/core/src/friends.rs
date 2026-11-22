//! Binder VPN's friends, kept by the sign-in service on the home server (`vpn/server/server.py`, the `vpn` service
//! of a compose project in `vpn_dir`). Everything goes over the server's SSH: names quoted, passwords and the OAuth
//! secret on stdin, never in a command line.

use crate::remote::{Remote, quote};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub name: String,
    pub created: i64,
    pub last_login: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Service {
    /// The service runs on the server.
    pub installed: bool,
    /// Its Tailscale OAuth client is set.
    pub configured: bool,
    pub users: usize,
}

const MISSING: &str = "--binder:missing--";

/// The compose folder under the server's home: plain relative path parts, never `..`.
pub fn check_dir(dir: &str) -> Result<()> {
    let ok = !dir.is_empty()
        && dir.len() <= 200
        && !dir.starts_with('/')
        && dir.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)));
    if ok { Ok(()) } else { Err(Error::Invalid("The service folder is a path under the server's home folder, like binder-vpn.".into())) }
}

fn script(dir: &str, args: &str) -> String {
    format!(
        "cd \"$HOME\"/{} 2>/dev/null && docker compose ps --status running -q vpn 2>/dev/null | grep -q . || {{ echo {MISSING}; exit 0; }}\n\
         exec docker compose exec -T vpn python /app/server.py {args}\n",
        quote(dir)
    )
}

const SERVER_PY: &str = include_str!("../../../vpn/server/server.py");
const DOCKERFILE: &str = include_str!("../../../vpn/server/Dockerfile");
/// The compose file for a folder of its own. The port is the server's alone; a public https address in front of it
/// (Tailscale Funnel, a Cloudflare Tunnel, a reverse proxy) is what friends reach.
const COMPOSE: &str = "# Binder VPN's sign-in service. Friends reach it through a public https address that forwards to
# 127.0.0.1:8787 on this server, such as `tailscale funnel --bg 8787` or a Cloudflare Tunnel.
services:
  vpn:
    build: ./vpn
    user: \"@UID@:@GID@\"
    ports:
      - \"127.0.0.1:8787:8080\"
    volumes:
      - ./vpn-data:/data
    read_only: true
    tmpfs:
      - /tmp
    cap_drop: [ALL]
    security_opt:
      - no-new-privileges:true
    restart: unless-stopped
";

/// Writes the service into its folder on the server and starts it, or updates it. A compose file already in the
/// folder (one the owner runs other things with) is kept; it needs a `vpn` service that builds `./vpn`.
pub fn install(remote: &dyn Remote, dir: &str) -> Result<()> {
    check_dir(dir)?;
    let d = quote(dir);
    let script = format!(
        "set -e\ncd \"$HOME\"\nmkdir -p -- {d}/vpn {d}/vpn-data\nchmod 700 -- {d}/vpn-data\ncd -- {d}\n\
         awk '/^--binder:file:server--$/{{f=\"vpn/server.py\";next}} /^--binder:file:dockerfile--$/{{f=\"vpn/Dockerfile\";next}} /^--binder:file:compose--$/{{f=\".compose.new\";next}} f{{print > f}}'\n\
         sed -i \"s/@UID@/$(id -u)/; s/@GID@/$(id -g)/\" .compose.new\n\
         if [ -e docker-compose.yml ] || [ -e docker-compose.yaml ] || [ -e compose.yml ] || [ -e compose.yaml ]; then rm -f .compose.new; else mv .compose.new docker-compose.yml; fi\n\
         out=$(docker compose up -d --build vpn 2>&1) || {{ printf '%s\\n' \"$out\" | tail -n 20 >&2; exit 1; }}\n"
    );
    let payload = format!("--binder:file:server--\n{SERVER_PY}\n--binder:file:dockerfile--\n{DOCKERFILE}\n--binder:file:compose--\n{COMPOSE}");
    remote.run(&script, Some(&payload)).map(drop)
}

/// The service's public address, as friends type it: https, a host, no trailing slash.
pub fn address(raw: &str) -> Result<String> {
    let a = raw.trim().trim_end_matches('/');
    let ok = a.strip_prefix("https://").is_some_and(|rest| {
        !rest.is_empty() && !rest.starts_with('/') && rest.chars().all(|c| c.is_ascii_graphic() && !"\"'\\<>`{}|^".contains(c))
    });
    if ok { Ok(a.to_string()) } else { Err(Error::Invalid("The address is an https address, like https://vpn.example.com.".into())) }
}

fn check_name(name: &str) -> Result<()> {
    let ok = (1..=32).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-');
    if ok { Ok(()) } else { Err(Error::Invalid("A name is 1 to 32 lowercase letters, digits or dashes.".into())) }
}

fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < 8 {
        return Err(Error::Invalid("A password is at least 8 characters.".into()));
    }
    if password.contains(['\n', '\r']) {
        return Err(Error::Invalid("A password is one line.".into()));
    }
    Ok(())
}

fn run(remote: &dyn Remote, dir: &str, args: &str, stdin: Option<&str>) -> Result<String> {
    check_dir(dir)?;
    let out = remote.run(&script(dir, args), stdin)?;
    if out.trim() == MISSING {
        return Err(Error::Invalid("Binder VPN's service isn't running on the server.".into()));
    }
    Ok(out)
}

pub fn status(remote: &dyn Remote, dir: &str) -> Result<Service> {
    check_dir(dir)?;
    let out = remote.run(&script(dir, "status"), None)?;
    if out.trim() == MISSING {
        return Ok(Service::default());
    }
    #[derive(Deserialize)]
    struct Raw {
        configured: bool,
        users: usize,
    }
    let raw: Raw = serde_json::from_str(out.trim()).map_err(|e| Error::Invalid(format!("Couldn't read the service: {e}")))?;
    Ok(Service { installed: true, configured: raw.configured, users: raw.users })
}

pub fn list(remote: &dyn Remote, dir: &str) -> Result<Vec<Friend>> {
    serde_json::from_str(run(remote, dir, "users", None)?.trim()).map_err(|e| Error::Invalid(format!("Couldn't read the friends: {e}")))
}

pub fn add(remote: &dyn Remote, dir: &str, name: &str, password: &str) -> Result<()> {
    check_name(name)?;
    check_password(password)?;
    run(remote, dir, &format!("add {}", quote(name)), Some(&format!("{password}\n"))).map(drop)
}

pub fn set_password(remote: &dyn Remote, dir: &str, name: &str, password: &str) -> Result<()> {
    check_name(name)?;
    check_password(password)?;
    run(remote, dir, &format!("password {}", quote(name)), Some(&format!("{password}\n"))).map(drop)
}

pub fn remove(remote: &dyn Remote, dir: &str, name: &str) -> Result<()> {
    check_name(name)?;
    run(remote, dir, &format!("remove {}", quote(name)), None).map(drop)
}

pub fn set_oauth(remote: &dyn Remote, dir: &str, id: &str, secret: &str) -> Result<()> {
    let (id, secret) = (id.trim(), secret.trim());
    if id.is_empty() || secret.is_empty() || id.contains(['\n', '\r']) || secret.contains(['\n', '\r']) {
        return Err(Error::Invalid("Both the client id and the secret are needed.".into()));
    }
    run(remote, dir, "oauth", Some(&format!("{id}\n{secret}\n"))).map(drop)
}
