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
  created: number;
  updated: number;
}

export interface NewTask {
  title: string;
  notes?: string;
  status?: Status;
  priority?: Priority;
  project_id?: number;
  assignee?: Assignee;
  due?: string;
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
}

export interface DoneLine {
  date: string;
  text: string;
}

export interface Config {
  vault: string;
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
