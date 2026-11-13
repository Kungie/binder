//! The records Binder keeps, as the interface sees them (serde, snake_case).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    #[default]
    Todo,
    InProgress,
    Done,
    Canceled,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Backlog => "backlog",
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::Done => "done",
            Status::Canceled => "canceled",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        [Status::Backlog, Status::Todo, Status::InProgress, Status::Done, Status::Canceled].into_iter().find(|v| v.as_str() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    #[default]
    None,
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub fn as_i64(self) -> i64 {
        self as i64
    }

    pub fn from_i64(n: i64) -> Priority {
        match n {
            1 => Priority::Low,
            2 => Priority::Medium,
            3 => Priority::High,
            4 => Priority::Urgent,
            _ => Priority::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Assignee {
    #[default]
    Me,
    Agent,
}

impl Assignee {
    pub fn as_str(self) -> &'static str {
        match self {
            Assignee::Me => "me",
            Assignee::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Assignee {
        if s == "agent" { Assignee::Agent } else { Assignee::Me }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    #[default]
    Active,
    Paused,
    Done,
}

impl ProjectState {
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectState::Active => "active",
            ProjectState::Paused => "paused",
            ProjectState::Done => "done",
        }
    }

    pub fn parse(s: &str) -> ProjectState {
        match s {
            "paused" => ProjectState::Paused,
            "done" => ProjectState::Done,
            _ => ProjectState::Active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub key: String,
    pub title: String,
    pub notes: String,
    pub status: Status,
    pub priority: Priority,
    pub project_id: Option<i64>,
    pub assignee: Assignee,
    /// `YYYY-MM-DD`
    pub due: Option<String>,
    /// `HH:MM`, only with a due date.
    pub due_time: Option<String>,
    /// Unix seconds.
    pub created: i64,
    pub updated: i64,
    /// Set when the agent's run ends; cleared when the owner opens the task.
    pub attention: bool,
    /// Order inside its status column, lower first.
    pub sort: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NewTask {
    pub title: String,
    pub notes: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub project_id: Option<i64>,
    pub assignee: Option<Assignee>,
    pub due: Option<String>,
    pub due_time: Option<String>,
}

/// What to change on a task. `None` leaves a field alone; the `clear_*` flags empty the optional ones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub project_id: Option<i64>,
    pub clear_project: bool,
    pub assignee: Option<Assignee>,
    pub due: Option<String>,
    /// Also clears `due_time`.
    pub clear_due: bool,
    pub due_time: Option<String>,
    pub clear_due_time: bool,
    pub attention: Option<bool>,
    pub sort: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum TaskView {
    All,
    Mine,
    Agent,
    Project(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub prefix: String,
    pub repo: Option<String>,
    /// Absolute path of the project's note, once Start project made it.
    pub note: Option<String>,
    pub state: ProjectState,
    pub created: i64,
    /// The folder on the home server where the agent runs this project's tasks.
    pub agent_dir: Option<String>,
    /// `owner/name` on GitHub.
    pub github: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub pushed_at: Option<String>,
    pub private: bool,
    /// Archived on GitHub, or gone from the owner's repos.
    pub archived: bool,
    /// One of `projects::PALETTE`.
    pub color: String,
    /// The repo's main language on GitHub.
    pub language: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NewProject {
    pub name: String,
    pub prefix: Option<String>,
    pub repo: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub repo: Option<String>,
    pub clear_repo: bool,
    pub state: Option<ProjectState>,
    /// Blank clears it.
    pub agent_dir: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    Mac,
    Server,
    Printer,
    #[default]
    Other,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Mac => "mac",
            DeviceKind::Server => "server",
            DeviceKind::Printer => "printer",
            DeviceKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> DeviceKind {
        match s {
            "mac" => DeviceKind::Mac,
            "server" => DeviceKind::Server,
            "printer" => DeviceKind::Printer,
            _ => DeviceKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: i64,
    pub name: String,
    pub kind: DeviceKind,
    pub address: String,
    pub port: u16,
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NewDevice {
    pub name: String,
    pub kind: DeviceKind,
    pub address: String,
    pub port: Option<u16>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevicePatch {
    pub name: Option<String>,
    pub kind: Option<DeviceKind>,
    pub address: Option<String>,
    pub port: Option<u16>,
    pub note: Option<String>,
}
