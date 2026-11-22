//! Binder VPN's friends, kept by the sign-in service on the home server (`vpn/server/server.py`, the `vpn` service
//! of the site's compose project). Everything goes over the server's SSH: names quoted, passwords and the OAuth
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

fn script(args: &str) -> String {
    format!(
        "cd \"$HOME/sites/binder-site\" 2>/dev/null && docker compose ps --status running -q vpn 2>/dev/null | grep -q . || {{ echo {MISSING}; exit 0; }}\n\
         exec docker compose exec -T vpn python /app/server.py {args}\n"
    )
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

fn run(remote: &dyn Remote, args: &str, stdin: Option<&str>) -> Result<String> {
    let out = remote.run(&script(args), stdin)?;
    if out.trim() == MISSING {
        return Err(Error::Invalid("Binder VPN's service isn't running on the server.".into()));
    }
    Ok(out)
}

pub fn status(remote: &dyn Remote) -> Result<Service> {
    let out = remote.run(&script("status"), None)?;
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

pub fn list(remote: &dyn Remote) -> Result<Vec<Friend>> {
    serde_json::from_str(run(remote, "users", None)?.trim()).map_err(|e| Error::Invalid(format!("Couldn't read the friends: {e}")))
}

pub fn add(remote: &dyn Remote, name: &str, password: &str) -> Result<()> {
    check_name(name)?;
    check_password(password)?;
    run(remote, &format!("add {}", quote(name)), Some(&format!("{password}\n"))).map(drop)
}

pub fn set_password(remote: &dyn Remote, name: &str, password: &str) -> Result<()> {
    check_name(name)?;
    check_password(password)?;
    run(remote, &format!("password {}", quote(name)), Some(&format!("{password}\n"))).map(drop)
}

pub fn remove(remote: &dyn Remote, name: &str) -> Result<()> {
    check_name(name)?;
    run(remote, &format!("remove {}", quote(name)), None).map(drop)
}

pub fn set_oauth(remote: &dyn Remote, id: &str, secret: &str) -> Result<()> {
    let (id, secret) = (id.trim(), secret.trim());
    if id.is_empty() || secret.is_empty() || id.contains(['\n', '\r']) || secret.contains(['\n', '\r']) {
        return Err(Error::Invalid("Both the client id and the secret are needed.".into()));
    }
    run(remote, "oauth", Some(&format!("{id}\n{secret}\n"))).map(drop)
}
