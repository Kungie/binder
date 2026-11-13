import type { Api } from "./api";
import type { Config, DoneLine, NewProject, Project, Task } from "./types";
import { localDay, oneLine, viewTasks } from "./tasks";

export interface Seed {
  projects: Project[];
  tasks: Task[];
  done: Record<number, DoneLine[]>;
}

export interface MemoryOptions {
  vaultConnected?: boolean;
}

const FOLD: Record<string, string> = { ş: "S", Ş: "S", ı: "I", İ: "I", ğ: "G", Ğ: "G", ü: "U", Ü: "U", ö: "O", Ö: "O", ç: "C", Ç: "C" };

export function derivePrefix(name: string, taken: string[]): string {
  const letters = Array.from(name)
    .map((c) => FOLD[c] ?? c)
    .filter((c) => /[a-z]/i.test(c))
    .slice(0, 3)
    .join("")
    .toUpperCase();
  const base = letters.length < 2 ? "PRJ" : letters;
  let candidate = base;
  for (let n = 2; taken.includes(candidate); n++) candidate = `${base}${n}`;
  return candidate;
}

/** An in-memory stand-in for the app, following the core's rules. Used by tests and the browser preview. */
export function memoryApi(seed: Seed, options: MemoryOptions = {}): Api & { done: Record<number, DoneLine[]> } {
  const tasks = seed.tasks.map((t) => ({ ...t }));
  const projects = seed.projects.map((p) => ({ ...p }));
  const done: Record<number, DoneLine[]> = Object.fromEntries(Object.entries(seed.done).map(([k, v]) => [k, [...v]]));
  const counters: Record<string, number> = {};
  let config: Config = { vault: "/Volumes/vault" };
  let nextId = Math.max(0, ...tasks.map((t) => t.id), ...projects.map((p) => p.id)) + 1;
  const now = () => Math.floor(Date.now() / 1000);
  const connected = () => options.vaultConnected ?? true;

  const nextNumber = (prefix: string) => {
    const used = tasks.filter((t) => t.key.startsWith(`${prefix}-`)).map((t) => Number(t.key.split("-").pop()));
    counters[prefix] = Math.max(counters[prefix] ?? 1, Math.max(0, ...used) + 1);
    return counters[prefix]++;
  };
  const project = (id: number) => {
    const p = projects.find((x) => x.id === id);
    if (!p) throw new Error(`no project with id ${id}`);
    return p;
  };
  const task = (id: number) => {
    const t = tasks.find((x) => x.id === id);
    if (!t) throw new Error(`no task with id ${id}`);
    return t;
  };
  const withNote = (p: Project) => {
    if (!connected()) throw new Error(`Vault not connected: ${config.vault}`);
    p.note ??= `${config.vault}/Projects/${p.name}.md`;
    done[p.id] ??= [];
    return p;
  };

  return {
    done,
    async listTasks(view, includeClosed) {
      return viewTasks(tasks, view, includeClosed).map((t) => ({ ...t }));
    },
    async createTask(t) {
      const title = t.title.trim();
      if (!title) throw new Error("A task needs a title.");
      const prefix = t.project_id ? project(t.project_id).prefix : "TSK";
      const created: Task = {
        id: nextId++,
        key: `${prefix}-${nextNumber(prefix)}`,
        title,
        notes: t.notes ?? "",
        status: t.status ?? "todo",
        priority: t.priority ?? "none",
        project_id: t.project_id ?? null,
        assignee: t.assignee ?? "me",
        due: t.due ?? null,
        created: now(),
        updated: now(),
      };
      tasks.push(created);
      return { ...created };
    },
    async updateTask(id, p) {
      const t = task(id);
      if (p.title !== undefined) {
        if (!p.title.trim()) throw new Error("A task needs a title.");
        t.title = p.title.trim();
      }
      if (p.notes !== undefined) t.notes = p.notes;
      if (p.status) t.status = p.status;
      if (p.priority) t.priority = p.priority;
      if (p.clear_project) t.project_id = null;
      else if (p.project_id !== undefined) t.project_id = project(p.project_id).id;
      if (p.assignee) t.assignee = p.assignee;
      if (p.clear_due) t.due = null;
      else if (p.due !== undefined) t.due = p.due;
      t.updated = Math.max(now(), t.updated + 1);
      return { ...t };
    },
    async deleteTask(id) {
      tasks.splice(tasks.indexOf(task(id)), 1);
    },
    async listProjects() {
      const rank = { active: 0, paused: 1, done: 2 };
      return [...projects].sort((a, b) => rank[a.state] - rank[b.state] || a.name.localeCompare(b.name)).map((p) => ({ ...p }));
    },
    async createProject(np: NewProject) {
      const name = np.name.trim();
      if (!name) throw new Error("A project needs a name.");
      if (projects.some((p) => p.name.toLowerCase() === name.toLowerCase())) throw new Error(`There is already a project called ${name}.`);
      const taken = projects.map((p) => p.prefix);
      let prefix = derivePrefix(name, taken);
      if (np.prefix?.trim()) {
        prefix = np.prefix.trim().toUpperCase();
        if (!/^[A-Z0-9]{2,5}$/.test(prefix)) throw new Error("A prefix is 2 to 5 letters or digits.");
        if (taken.includes(prefix)) throw new Error(`The prefix ${prefix} is taken.`);
      }
      const p: Project = { id: nextId++, name, prefix, repo: np.repo ?? null, note: null, state: "active", created: now() };
      projects.push(p);
      return { ...p };
    },
    async updateProject(id, patch) {
      const p = project(id);
      if (patch.name?.trim()) p.name = patch.name.trim();
      if (patch.clear_repo) p.repo = null;
      else if (patch.repo !== undefined) p.repo = patch.repo;
      if (patch.state) p.state = patch.state;
      return { ...p };
    },
    async deleteProject(id) {
      projects.splice(projects.indexOf(project(id)), 1);
      for (const t of tasks) if (t.project_id === id) t.project_id = null;
    },
    async startProject(id) {
      return { ...withNote(project(id)) };
    },
    async openNote(id) {
      withNote(project(id));
    },
    async logDone(projectId, text) {
      const p = withNote(project(projectId));
      const line = oneLine(text);
      if (!line) return false;
      done[p.id].push({ date: localDay(), text: line });
      return true;
    },
    async readDone(projectId) {
      return [...(done[projectId] ?? [])];
    },
    async doneToday() {
      const day = localDay();
      return projects.flatMap((p) => (done[p.id] ?? []).filter((l) => l.date === day).map((l) => [p.name, l] as [string, DoneLine]));
    },
    async getConfig() {
      return { ...config };
    },
    async setConfig(c) {
      config = { ...c };
    },
    async vaultConnected() {
      return connected();
    },
  };
}

/** A made-up developer's workshop, for the preview and tests. */
export function demoSeed(): Seed {
  const t = Math.floor(Date.now() / 1000);
  const day = localDay();
  const projects: Project[] = [
    { id: 1, name: "orbit", prefix: "ORB", repo: "~/code/orbit", note: "/Volumes/vault/Projects/orbit.md", state: "active", created: t - 864000 },
    { id: 2, name: "homelab", prefix: "LAB", repo: null, note: null, state: "active", created: t - 432000 },
  ];
  const mk = (id: number, key: string, title: string, extra: Partial<Task>): Task => ({
    id: id + 10,
    key,
    title,
    notes: "",
    status: "todo",
    priority: "none",
    project_id: null,
    assignee: "me",
    due: null,
    created: t - id * 3600,
    updated: t - id * 600,
    ...extra,
  });
  const tasks: Task[] = [
    mk(1, "ORB-4", "Wire the sync status into the toolbar", { project_id: 1, status: "in_progress", priority: "high" }),
    mk(2, "LAB-2", "Update all containers and prune old images", { project_id: 2, assignee: "agent", priority: "medium" }),
    mk(3, "ORB-3", "Write release notes for 0.4", { project_id: 1, due: day }),
    mk(4, "TSK-1", "Print a new case lid for the Pi", { priority: "low" }),
    mk(5, "LAB-1", "Back up the Home Assistant config", { project_id: 2, assignee: "agent", status: "done" }),
    mk(6, "ORB-2", "Try a lighter font for the sidebar", { project_id: 1, status: "backlog" }),
    mk(7, "TSK-2", "Order PETG, black and white", { priority: "urgent" }),
    mk(8, "ORB-1", "Set up the project", { project_id: 1, status: "done" }),
  ];
  return {
    projects,
    tasks,
    done: { 1: [{ date: day, text: "Keep sync off by default until conflicts are handled." }] },
  };
}
