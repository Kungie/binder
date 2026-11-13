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

const DAY = 86_400_000;
const dayValue = (d: string) => Date.UTC(Number(d.slice(0, 4)), Number(d.slice(5, 7)) - 1, Number(d.slice(8, 10)));

/** "14:00" → "2 PM", "09:30" → "9:30 AM". */
function clock(time: string): string {
  const [h, m] = time.split(":").map(Number);
  const hour = h % 12 === 0 ? 12 : h % 12;
  return `${hour}${m ? `:${String(m).padStart(2, "0")}` : ""} ${h < 12 ? "AM" : "PM"}`;
}

/** A due chip's words: Today, Tomorrow, a weekday within the week, else the date; with the time when set. */
export function dueLabel(due: string, dueTime: string | null, today: string): { text: string; overdue: boolean } {
  const days = Math.round((dayValue(due) - dayValue(today)) / DAY);
  const date = new Date(dayValue(due));
  let text: string;
  if (days === 0) text = "Today";
  else if (days === 1) text = "Tomorrow";
  else if (days > 1 && days < 7) text = date.toLocaleDateString("en-GB", { weekday: "short", timeZone: "UTC" });
  else {
    const sameYear = due.slice(0, 4) === today.slice(0, 4);
    text = date.toLocaleDateString("en-GB", { day: "numeric", month: "short", ...(sameYear ? {} : { year: "numeric" }), timeZone: "UTC" });
  }
  return { text: dueTime ? `${text} ${clock(dueTime)}` : text, overdue: days < 0 };
}

/** Notes as plain words for a card: Markdown marks, links and code fences gone, whitespace collapsed. */
export function plainText(md: string): string {
  return md
    .replace(/```[^\n]*\n?/g, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, "")
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+[.)])\s+/gm, "")
    .replace(/(\*\*|__|\*|_|~~|`)/g, "")
    .split(/\s+/)
    .filter(Boolean)
    .join(" ");
}

/** Matches a search against a task's title and notes, ignoring case. */
export function matches(task: Task, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || task.title.toLowerCase().includes(q) || task.notes.toLowerCase().includes(q);
}
