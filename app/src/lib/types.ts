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
}

export type TaskView = { kind: "all" } | { kind: "mine" } | { kind: "agent" } | { kind: "project"; id: number };

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
}

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

export type Access = "edits_only" | "full";

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
  entry: CloudEntry;
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
  done: number;
  total: number;
  state: TransferState;
  error: string | null;
}
