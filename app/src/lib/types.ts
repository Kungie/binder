// Mirrors crates/core/src/model.rs and notes.rs. Field names stay snake_case, as serde sends them.

export type Status = "backlog" | "todo" | "in_progress" | "done" | "canceled";
export type Priority = "none" | "low" | "medium" | "high" | "urgent";
export type Assignee = "me" | "agent";
export type ProjectState = "active" | "paused" | "done";

export interface Task {
  id: number;
  key: string;
  title: string;
  notes: string;
  status: Status;
  priority: Priority;
  project_id: number | null;
  assignee: Assignee;
  due: string | null;
  /** `HH:MM` */
  due_time: string | null;
  created: number;
  updated: number;
  attention: boolean;
  sort: number;
  /** Runs on the server from 03:00 instead of being picked up during the day. */
  night: boolean;
  /** When it moved to Done or Canceled, Unix seconds; null while open and for tasks closed before 2.7. */
  closed: number | null;
  /** The GitHub issue it came from. */
  issue_number: number | null;
  issue_url: string | null;
}

export interface NewTask {
  title: string;
  notes?: string;
  status?: Status;
  priority?: Priority;
  project_id?: number;
  assignee?: Assignee;
  due?: string;
  due_time?: string;
  /** A night task, which also goes to the agent. */
  night?: boolean;
}

export interface TaskPatch {
  title?: string;
  notes?: string;
  status?: Status;
  priority?: Priority;
  project_id?: number;
  clear_project?: boolean;
  assignee?: Assignee;
  due?: string;
  clear_due?: boolean;
  due_time?: string;
  clear_due_time?: boolean;
  attention?: boolean;
  sort?: number;
  /** Marking a task for the night also assigns it to the agent. */
  night?: boolean;
}

export type TaskView = { kind: "all" } | { kind: "mine" } | { kind: "agent" } | { kind: "night" } | { kind: "project"; id: number };

export interface Project {
  id: number;
  name: string;
  prefix: string;
  repo: string | null;
  note: string | null;
  state: ProjectState;
  created: number;
  agent_dir: string | null;
  github: string | null;
  url: string | null;
  description: string | null;
  pushed_at: string | null;
  private: boolean;
  archived: boolean;
  color: string;
  language: string | null;
  /** Open pull requests on GitHub, null before the first read. */
  open_prs: number | null;
  /** The latest checks on the default branch; null when it has none. */
  ci: Ci | null;
}

export type Ci = "passing" | "failing" | "running";

export interface NewProject {
  name: string;
  prefix?: string;
  repo?: string;
}

export interface ProjectPatch {
  name?: string;
  repo?: string;
  clear_repo?: boolean;
  state?: ProjectState;
  /** Blank clears it. */
  agent_dir?: string;
  color?: string;
}

export interface DoneLine {
  date: string;
  /** `HH:MM`, local. Lines from a note's old `## Done` section have none. */
  time: string | null;
  text: string;
}

/** A done line with its project, for Overview's Done today. */
export type GhFilter = "needs_you" | "mine" | "all_open";
export type Review = "approved" | "changes_requested" | "review_required" | "draft";

export interface PullRow {
  repo: string;
  number: number;
  title: string;
  author: string;
  ci: Ci | null;
  review: Review | null;
  review_requested: boolean;
  mine: boolean;
  /** RFC 3339. */
  updated: string;
}

export interface IssueRow {
  repo: string;
  number: number;
  title: string;
  author: string;
  assignees: string[];
  updated: string;
}

export interface GhLists {
  login: string;
  pulls: PullRow[];
  issues: IssueRow[];
}

export interface GhComment {
  author: string;
  body: string;
  at: string;
  /** A review's verdict; null for a plain comment. */
  state: string | null;
}

export interface GhDetail {
  title: string;
  body: string;
  head: string | null;
  base: string | null;
  additions: number;
  deletions: number;
  files: number;
  /** [name, result] */
  checks: [string, Ci | null][];
  comments: GhComment[];
  url: string;
}

export type GhAction =
  | { kind: "comment"; body: string }
  | { kind: "approve"; body: string }
  | { kind: "request_changes"; body: string }
  | { kind: "merge" }
  | { kind: "close" }
  | { kind: "reopen" }
  | { kind: "assign_me" };

/** Whether the agent can work on a project's GitHub: gh signed in on the server, and an agent folder. */
export interface AgentReady {
  gh: boolean;
  folder: boolean;
}

export interface SiteContainer {
  name: string;
  state: string;
}

/** A website on the home server: a folder in ~/sites with a docker-compose.yml. */
export interface Site {
  name: string;
  /** https://…; null when none was found or saved. */
  address: string | null;
  containers: SiteContainer[];
  cpu: number;
  mem_bytes: number;
  disk_bytes: number;
}

export interface SiteCheck {
  status: number | null;
  frameable: boolean;
  error: string | null;
}

/** One line of the morning brief; the interface words it. Times are Unix seconds. */
export type BriefItem =
  | { kind: "run"; task_id: number; key: string; ok: boolean; night: boolean; at: number }
  | { kind: "container"; name: string; problem: "stopped" | "restarting" | "unhealthy"; at: number; back_at: number | null }
  | { kind: "server_restart"; at: number }
  | { kind: "print"; job: string; ok: boolean; at: number }
  | { kind: "issue"; task_id: number; key: string; title: string; at: number };

export interface Brief {
  since: number;
  /** Between 05:00 and noon and not dismissed this morning. */
  shown: boolean;
  items: BriefItem[];
}

export interface DoneItem {
  project_id: number;
  project: string;
  color: string;
  line: DoneLine;
}

export interface PrinterConfig {
  name: string;
  address: string;
  serial: string;
  cert_sha256: string | null;
}

export interface Config {
  vault: string;
  server: string | null;
  printers: PrinterConfig[];
  claude_hooks: boolean;
  cloud: string;
  cloud_share: string | null;
  /** The shared folder's path on the server, for the SSH fast path. */
  cloud_remote: string;
  /** Whether Binder starts the agent's tasks by itself. */
  pick_up: boolean;
  /** Whether Binder watches the server's containers and sends notifications. */
  watch: boolean;
  /** The interface size in percent: 100, 110, 125 or 150. */
  zoom: number;
  /** Whether a finished or failed print sends a notification. */
  print_notify: boolean;
  /** Addresses the owner gave sites Binder couldn't find one for, by folder name. */
  site_addresses: Record<string, string>;
  /** Overview's two columns, widget ids from top to bottom, with their sizes. */
  overview: OverviewLayout;
}

export interface OverviewLayout {
  left: string[];
  right: string[];
  /** How wide the left column is, in percent (30 to 70). */
  split?: number;
  /** Heights in pixels of the widgets the owner sized; the others fit their content. */
  heights?: Record<string, number>;
}

export type DeviceKind = "mac" | "server" | "printer" | "other";

export interface Device {
  id: number;
  name: string;
  kind: DeviceKind;
  address: string;
  port: number;
  note: string;
}

export interface NewDevice {
  name: string;
  kind?: DeviceKind;
  address: string;
  port?: number;
  note?: string;
}

export interface DevicePatch {
  name?: string;
  kind?: DeviceKind;
  address?: string;
  port?: number;
  note?: string;
}

export const DEVICE_KIND_LABEL: Record<DeviceKind, string> = { mac: "Mac", server: "Server", printer: "Printer", other: "Other" };

export interface Container {
  name: string;
  image: string;
  state: string;
  status: string;
}

export interface Service {
  name: string;
  description: string;
}

export interface ServerStatus {
  containers: Container[];
  services: Service[];
  load: [number, number, number];
  mem_total: number;
  mem_available: number;
  disk_size: number;
  disk_used: number;
  temp_c: number | null;
  /** The Pi board's draw in watts, from its PMIC; null when it has none. */
  power_w: number | null;
  /** The SD card inside Cloud, when it is a disk of its own. */
  sd_size: number | null;
  sd_used: number | null;
  /** From nproc; 0 when unknown. */
  cores: number;
  at: number;
}

export interface PrinterState {
  gcode_state: string | null;
  percent: number | null;
  remaining_min: number | null;
  layer: number | null;
  total_layers: number | null;
  nozzle_c: number | null;
  nozzle_target_c: number | null;
  bed_c: number | null;
  bed_target_c: number | null;
  job: string | null;
}

export interface PrinterView {
  config: PrinterConfig;
  state: PrinterState;
  online: boolean;
  error: string | null;
}

export interface NewPrinter {
  name: string;
  address: string;
  serial: string;
  accessCode: string;
}

export type PrinterCommand = "pause" | "resume" | "stop";

/** A job is on the plate: printing, paused or getting ready. */
export const isPrinting = (s: PrinterState) => ["RUNNING", "PAUSE", "PREPARE"].includes(s.gcode_state ?? "");

export type Access = "edits_only" | "full" | "read_only";

export interface AgentRun {
  id: number;
  task_id: number;
  run: string;
  dir: string;
  access: Access;
  started: number;
  ended: number | null;
  exit_code: number | null;
  tail: string;
  /** A night run still waiting in the server's queue. */
  queued: boolean;
}

export interface RepoStatus {
  branch: string | null;
  changed: number;
  ahead: number;
  behind: number;
  last_subject: string | null;
  last_date: string | null;
}

export const STATUSES: Status[] = ["backlog", "todo", "in_progress", "done", "canceled"];
export const PRIORITIES: Priority[] = ["none", "low", "medium", "high", "urgent"];

export const STATUS_LABEL: Record<Status, string> = {
  backlog: "Backlog",
  todo: "Todo",
  in_progress: "In progress",
  done: "Done",
  canceled: "Canceled",
};

export const PRIORITY_LABEL: Record<Priority, string> = {
  none: "No priority",
  low: "Low priority",
  medium: "Medium priority",
  high: "High priority",
  urgent: "Urgent",
};

export const isClosed = (s: Status) => s === "done" || s === "canceled";

// GitHub, mirrors crates/core/src/github.rs.

export interface Repo {
  name: string;
  name_with_owner: string;
  description: string | null;
  url: string;
  is_private: boolean;
  is_archived: boolean;
  /** RFC 3339 */
  pushed_at: string;
  language: string | null;
}

/** A repo the owner deleted as a project; sync leaves it out until it's brought back. */
export interface HiddenRepo {
  github_id: string | null;
  name_with_owner: string;
  /** Unix seconds */
  hidden_at: number;
}

/** The last GitHub sync: when it ran and, if it failed, why. */
export interface GithubSync {
  /** Unix seconds */
  at: number;
  error: string | null;
}

export interface SyncReport {
  added: number;
  linked: number;
  updated: number;
  archived: number;
  issues_added?: number;
  issues_closed?: number;
}

// Cloud, mirrors crates/core/src/cloud.rs and app/src-tauri/src/transfers.rs.

export type EntryKind = "file" | "dir" | "link";
export type Conflict = "replace" | "keep_both" | "skip";

export interface CloudEntry {
  name: string;
  /** Relative to Cloud's root, `/`-separated. */
  rel: string;
  kind: EntryKind;
  size: number;
  /** Unix seconds */
  modified: number;
}

export interface TrashEntry {
  id: string;
  /** Where it was, relative to the root. */
  origin: string;
  /** Unix seconds */
  deleted: number;
  /** A copy that never finished; it can't be restored. */
  unfinished: boolean;
  entry: CloudEntry;
}

/** What Get info shows. */
export interface CloudInfo {
  entry: CloudEntry;
  /** Unix seconds; 0 when the share doesn't say. */
  created: number;
  /** The item's path on this Mac. */
  path: string;
}

export interface AppChoice {
  name: string;
  path: string;
  default: boolean;
}

export interface Space {
  total: number;
  free: number;
}

export interface CloudStatus {
  mounted: boolean;
  path: string;
  space: Space | null;
}

export type TransferState = "running" | "done" | "cancelled" | "failed";

export interface Transfer {
  id: number;
  name: string;
  /** What done and total count: bytes for copies, items for zips. */
  unit: "bytes" | "items";
  done: number;
  total: number;
  state: TransferState;
  error: string | null;
}

/** The Agents screen's two chats, each a tmux session on the home server. */
export type Agent = "claude" | "qwen";

/** The running app, as the updater sees it. */
export interface AppInfo {
  version: string;
  /** False in development builds, which never update. */
  updates: boolean;
}

/** A newer release on GitHub. */
export interface UpdateInfo {
  version: string;
  /** The release notes, as written (Markdown). */
  notes: string;
}

export type InstallProgress = { kind: "downloading"; percent: number | null } | { kind: "installing" };

/** One of the processes using the most CPU or memory on the server. */
export interface TopProcess {
  name: string;
  cpu: number;
  /** Share of memory, in percent. */
  mem: number;
  rss_mb: number;
}

/** One of the largest folders on the server's disks. */
export interface TopFolder {
  path: string;
  bytes: number;
}
