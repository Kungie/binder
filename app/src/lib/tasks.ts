import { type Task, type TaskView, isClosed } from "./types";

const STATUS_RANK = { in_progress: 0, todo: 1, backlog: 2, done: 3, canceled: 4 } as const;
const PRIORITY_RANK = { urgent: 0, high: 1, medium: 2, low: 3, none: 4 } as const;

/** The core's order: open work first, most urgent first, most recently touched first. */
export function sortTasks(tasks: Task[]): Task[] {
  return [...tasks].sort(
    (a, b) =>
      STATUS_RANK[a.status] - STATUS_RANK[b.status] ||
      PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority] ||
      b.updated - a.updated ||
      b.id - a.id,
  );
}

export function viewTasks(tasks: Task[], view: TaskView, includeClosed: boolean): Task[] {
  return sortTasks(
    tasks.filter((t) => {
      if (!includeClosed && isClosed(t.status)) return false;
      switch (view.kind) {
        case "mine":
          return t.assignee === "me";
        case "agent":
          return t.assignee === "agent";
        case "project":
          return t.project_id === view.id;
        default:
          return true;
      }
    }),
  );
}

/** Collapses whitespace and caps at 200 characters, like the core's `one_line`. */
export function oneLine(text: string): string | null {
  const t = text.split(/\s+/).filter(Boolean).join(" ");
  return t ? Array.from(t).slice(0, 200).join("") : null;
}

export function localDay(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
