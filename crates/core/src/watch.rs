//! The container watch: each check of the server's containers becomes the alerts worth a notification, one when
//! a problem starts and one when it ends.

use crate::server::Container;
use std::collections::HashMap;

/// Checks in a row that can't reach the server before it counts as down.
const MISSES: u32 = 3;
/// Clean checks in a row before a container in trouble counts as back, so a crash loop that is sometimes
/// caught running doesn't send an alert each minute.
const CLEAR: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alert {
    /// A container seen running has stopped (or was removed): its name and status line.
    Stopped(String, String),
    Restarting(String, String),
    Unhealthy(String, String),
    Running(String),
    Unreachable,
    Back,
}

impl Alert {
    pub fn title(&self) -> String {
        match self {
            Alert::Stopped(n, _) => format!("{n} stopped"),
            Alert::Restarting(n, _) => format!("{n} keeps restarting"),
            Alert::Unhealthy(n, _) => format!("{n} is unhealthy"),
            Alert::Running(n) => format!("{n} is running again"),
            Alert::Unreachable => "Can't reach the server".into(),
            Alert::Back => "The server is back".into(),
        }
    }

    pub fn body(&self) -> String {
        match self {
            Alert::Stopped(_, s) | Alert::Restarting(_, s) | Alert::Unhealthy(_, s) => s.clone(),
            Alert::Running(_) | Alert::Back => String::new(),
            Alert::Unreachable => "Binder couldn't reach the home server three times in a row.".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trouble {
    Stopped,
    Restarting,
    Unhealthy,
}

#[derive(Debug, Default)]
struct Seen {
    /// Seen running at some check, so stopping now is news.
    was_up: bool,
    trouble: Option<Trouble>,
    /// Clean checks in a row while still counted in trouble.
    clean: u32,
}

#[derive(Debug, Default)]
pub struct Watch {
    containers: HashMap<String, Seen>,
    misses: u32,
}

fn trouble(c: &Container, was_up: bool) -> Option<Trouble> {
    match c.state.as_str() {
        "restarting" => Some(Trouble::Restarting),
        "running" if c.status.contains("(unhealthy)") => Some(Trouble::Unhealthy),
        "exited" | "dead" if was_up => Some(Trouble::Stopped),
        _ => None,
    }
}

fn alert(t: Trouble, c: &Container) -> Alert {
    let (n, s) = (c.name.clone(), c.status.clone());
    match t {
        Trouble::Stopped => Alert::Stopped(n, s),
        Trouble::Restarting => Alert::Restarting(n, s),
        Trouble::Unhealthy => Alert::Unhealthy(n, s),
    }
}

impl Watch {
    /// A check that read the containers. The first one only sets what counts as normal, except for containers
    /// already restarting or unhealthy.
    pub fn seen(&mut self, containers: &[Container]) -> Vec<Alert> {
        let mut out = vec![];
        if self.misses >= MISSES {
            out.push(Alert::Back);
        }
        self.misses = 0;
        let mut next = HashMap::new();
        for c in containers {
            let before = self.containers.remove(&c.name).unwrap_or_default();
            let was_up = before.was_up || c.state == "running";
            let seen = match (before.trouble, trouble(c, before.was_up)) {
                (None, None) => Seen { was_up, ..Default::default() },
                (None, Some(t)) => {
                    out.push(alert(t, c));
                    Seen { was_up, trouble: Some(t), clean: 0 }
                }
                // Still in trouble, of whatever kind: the alert already went out.
                (Some(t), Some(_)) => Seen { was_up, trouble: Some(t), clean: 0 },
                (Some(_), None) if before.clean + 1 >= CLEAR => {
                    out.push(Alert::Running(c.name.clone()));
                    Seen { was_up, ..Default::default() }
                }
                (Some(t), None) => Seen { was_up, trouble: Some(t), clean: before.clean + 1 },
            };
            next.insert(c.name.clone(), seen);
        }
        // A container seen running that is gone from the list was removed; it stays known as stopped, so its
        // return says so.
        let mut gone: Vec<_> = self.containers.drain().filter(|(_, s)| s.was_up).collect();
        gone.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, s) in gone {
            if s.trouble.is_none() {
                out.push(Alert::Stopped(name.clone(), "Removed".into()));
            }
            next.insert(name, Seen { was_up: true, trouble: Some(s.trouble.unwrap_or(Trouble::Stopped)), clean: 0 });
        }
        self.containers = next;
        out
    }

    /// A check that couldn't reach the server: the third in a row says so, once.
    pub fn missed(&mut self) -> Vec<Alert> {
        self.misses += 1;
        if self.misses == MISSES { vec![Alert::Unreachable] } else { vec![] }
    }
}
