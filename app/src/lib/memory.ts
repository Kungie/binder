import type { Api } from "./api";
import type { HiddenRepo, AgentRun, PrinterCommand, PrinterView, Config, Device, DoneItem, DoneLine, NewProject, Project, Repo, RepoStatus, ServerStatus, SyncReport, Task } from "./types";
import { type CloudSeed, memoryCloud } from "./memoryCloud";
import { localDay, oneLine, viewTasks } from "./tasks";
import { PALETTE, colorFor } from "./colors";

export interface Seed {
  projects: Project[];
  tasks: Task[];
  /** Each project's done lines, oldest first. */
  done: Record<number, DoneLine[]>;
  /** When a done line was last written, Unix seconds. */
  lastDone?: number | null;
  devices?: Device[];
  server?: ServerStatus;
  repos?: Record<number, RepoStatus>;
  printers?: PrinterView[];
  /** What the fake gh returns; null means it isn't signed in. */
  github?: Repo[] | null;
  cloud?: CloudSeed;
}

export interface MemoryOptions {
  /** Whether the Cloud share is mounted. Defaults to true. */
  cloudMounted?: boolean;
  vaultConnected?: boolean;
  /** Whether Settings › Claude Code starts on. */
  claudeHooks?: boolean;
  /** The SSH target in the config. Defaults to "lab"; null means none is set. */
  server?: string | null;
  /** Device addresses that don't answer a probe. */
  offline?: string[];
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
export function memoryApi(
  seed: Seed,
  options: MemoryOptions = {},
): Api & {
  done: Record<number, DoneLine[]>;
  setServerReachable(v: boolean): void;
  printerCommands: [string, PrinterCommand][];
  cloudOpened: string[];
  /** Items Quick Look was asked to show. */
  quickLooked: string[];
  /** URLs opened in the browser. */
  opened: string[];
  readonly cloudConnects: number;
  /** Acts as if files were dropped on the window at a point. */
  dropFiles(paths: string[], x?: number, y?: number): void;
  /** Mac paths each native drag started with. */
  dragged: string[][];
} {
  const tasks = seed.tasks.map((t) => ({ ...t }));
  const projects = seed.projects.map((p) => ({ ...p }));
  const done: Record<number, DoneLine[]> = Object.fromEntries(Object.entries(seed.done).map(([k, v]) => [k, [...v]]));
  const counters: Record<string, number> = {};
  const devices = (seed.devices ?? []).map((d) => ({ ...d }));
  const runs: AgentRun[] = [];
  const printers: PrinterView[] = (seed.printers ?? []).map((p) => ({ ...p, config: { ...p.config }, state: { ...p.state } }));
  const printerCommands: [string, PrinterCommand][] = [];
  const printer = (serial: string) => {
    const p = printers.find((x) => x.config.serial === serial);
    if (!p) throw new Error(`No printer ${serial}.`);
    return p;
  };
  const findRun = (run: string) => {
    const r = runs.find((x) => x.run === run);
    if (!r) throw new Error(`No run called ${run}.`);
    return r;
  };
  const offline = options.offline ?? ["10.0.7.30"];
  let reachable = true;
  let config: Config = { vault: "/Volumes/vault", server: options.server === undefined ? "lab" : options.server, printers: [], claude_hooks: options.claudeHooks ?? false, cloud: "/Volumes/cloud", cloud_share: null, cloud_remote: "/srv/cloud" };
  let lastDone: number | null = seed.lastDone ?? null;
  let nextId = Math.max(0, ...tasks.map((t) => t.id), ...projects.map((p) => p.id), ...devices.map((d) => d.id)) + 1;
  const device = (id: number) => {
    const d = devices.find((x) => x.id === id);
    if (!d) throw new Error(`no device with id ${id}`);
    return d;
  };
  const required = (what: string, v: string) => {
    if (!v.trim()) throw new Error(`A device needs ${what}.`);
    return v.trim();
  };
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

  const opened: string[] = [];
  let hidden: HiddenRepo[] = [];
  const isHidden = (r: Repo) => hidden.some((h) => h.name_with_owner.toLowerCase() === r.name_with_owner.toLowerCase());
  const cloud = memoryCloud(seed.cloud, { mounted: () => options.cloudMounted ?? true, share: () => config.cloud_share });

  /** The core's sync rules: link by name, add new as active or paused, update facts only, archive what's gone. */
  const githubSync = async (): Promise<SyncReport> => {
    const repos = seed.github === undefined ? [] : seed.github;
    if (repos === null) throw new Error("Sign in with gh auth login to bring in your GitHub repos.");
    const report: SyncReport = { added: 0, linked: 0, updated: 0, archived: 0 };
    for (const r of repos) {
      if (isHidden(r)) continue;
      let p = projects.find((x) => x.github === r.name_with_owner);
      if (p) {
        const same = p.url === r.url && p.description === r.description && p.pushed_at === r.pushed_at && p.private === r.is_private && p.archived === r.is_archived && p.language === r.language;
        if (!same) {
          if (r.is_archived && !p.archived) report.archived++;
          else report.updated++;
        }
      } else if ((p = projects.find((x) => !x.github && x.name.toLowerCase() === r.name.toLowerCase()))) {
        report.linked++;
      } else {
        const fresh = Date.now() - Date.parse(r.pushed_at) <= 60 * 86400000;
        p = { ...hand(r.name), id: nextId++, prefix: derivePrefix(r.name, projects.map((x) => x.prefix)), created: now(), state: fresh ? "active" : "paused" };
        projects.push(p);
        report.added++;
      }
      Object.assign(p, { github: r.name_with_owner, url: r.url, description: r.description, pushed_at: r.pushed_at, private: r.is_private, archived: r.is_archived, language: r.language });
    }
    for (const p of projects) {
      if (p.github && !p.archived && !repos.some((r) => r.name_with_owner === p.github)) {
        p.archived = true;
        report.archived++;
      }
    }
    return report;
  };

  return {
    ...cloud,
    // A spread copies a getter's value once; this one must stay live.
    get cloudConnects() {
      return cloud.cloudConnects;
    },
    opened,
    githubSync,
    async hiddenRepos() {
      return hidden.map((h) => ({ ...h }));
    },
    async unhideRepo(nameWithOwner) {
      hidden = hidden.filter((h) => h.name_with_owner.toLowerCase() !== nameWithOwner.toLowerCase());
    },
    async openRepoPage(projectId) {
      const p = project(projectId);
      if (!p.url) throw new Error("This project isn't on GitHub.");
      opened.push(p.url);
    },
    onProjectsChanged() {
      return () => {};
    },
    done,
    printerCommands,
    setServerReachable(v) {
      reachable = v;
    },
    async listTasks(view, includeClosed) {
      return viewTasks(tasks, view, includeClosed).map((t) => ({ ...t }));
    },
    async createTask(t) {
      const title = t.title.trim();
      if (!title) throw new Error("A task needs a title.");
      const prefix = t.project_id ? project(t.project_id).prefix : "TSK";
      const status = t.status ?? "todo";
      const sort = Math.max(0, ...tasks.filter((x) => x.status === status).map((x) => x.sort)) + 1;
      const created: Task = {
        id: nextId++,
        key: `${prefix}-${nextNumber(prefix)}`,
        title,
        notes: t.notes ?? "",
        status,
        priority: t.priority ?? "none",
        project_id: t.project_id ?? null,
        assignee: t.assignee ?? "me",
        due: t.due ?? null,
        due_time: t.due ? (t.due_time ?? null) : null,
        created: now(),
        updated: now(),
        attention: false,
        sort,
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
      if (p.clear_due) (t.due = null), (t.due_time = null);
      else if (p.due !== undefined) t.due = p.due;
      if (p.clear_due_time) t.due_time = null;
      else if (p.due_time !== undefined) {
        if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(p.due_time)) throw new Error("A time is HH:MM.");
        t.due_time = p.due_time;
      }
      if (p.attention !== undefined) t.attention = p.attention;
      if (p.sort !== undefined) t.sort = p.sort;
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
      const taken = [...projects.map((p) => p.prefix), "TSK"];
      let prefix = derivePrefix(name, taken);
      if (np.prefix?.trim()) {
        prefix = np.prefix.trim().toUpperCase();
        if (!/^[A-Z0-9]{2,5}$/.test(prefix)) throw new Error("A prefix is 2 to 5 letters or digits.");
        if (prefix === "TSK") throw new Error("TSK is kept for tasks without a project.");
        if (taken.includes(prefix)) throw new Error(`The prefix ${prefix} is taken.`);
      }
      const p: Project = {
        id: nextId++,
        name,
        prefix,
        repo: np.repo ?? null,
        note: null,
        state: "active",
        created: now(),
        agent_dir: null,
        github: null,
        url: null,
        description: null,
        pushed_at: null,
        private: false,
        archived: false,
        color: colorFor(name),
        language: null,
      };
      projects.push(p);
      return { ...p };
    },
    async updateProject(id, patch) {
      const p = project(id);
      if (patch.name?.trim()) p.name = patch.name.trim();
      if (patch.clear_repo) p.repo = null;
      else if (patch.repo !== undefined) p.repo = patch.repo;
      if (patch.state) p.state = patch.state;
      if (patch.agent_dir !== undefined) p.agent_dir = patch.agent_dir.trim() || null;
      if (patch.color !== undefined) {
        if (!(PALETTE as readonly string[]).includes(patch.color)) throw new Error("Pick one of the project colours.");
        p.color = patch.color;
      }
      return { ...p };
    },
    async deleteProject(id) {
      const p = project(id);
      if (p.github) hidden.unshift({ github_id: null, name_with_owner: p.github, hidden_at: now() });
      projects.splice(projects.indexOf(p), 1);
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
      const now = new Date();
      const time = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
      done[p.id].push({ date: localDay(), time, text: line });
      lastDone = Math.floor(now.getTime() / 1000);
      return true;
    },
    async readDone(projectId) {
      const lines = [...(done[projectId] ?? [])];
      // Dated lines newest day first; within a day the later line is newer.
      return lines.map((l, i) => ({ l, i })).sort((a, b) => b.l.date.localeCompare(a.l.date) || b.i - a.i).map(({ l }) => ({ ...l }));
    },
    async doneToday() {
      const day = localDay();
      const items: DoneItem[] = projects.flatMap((p) =>
        (done[p.id] ?? []).filter((l) => l.date === day).map((l) => ({ project_id: p.id, project: p.name, color: p.color, line: { ...l } })),
      );
      return items.reverse().sort((a, b) => (b.line.time ?? "").localeCompare(a.line.time ?? ""));
    },
    async lastDone() {
      return lastDone;
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
    async serverStatus() {
      if (!config.server) throw new Error("No server set.");
      if (!reachable || !seed.server) throw new Error(`Can't reach ${config.server}`);
      return { ...seed.server, at: now() };
    },
    async listDevices() {
      return [...devices].sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" })).map((d) => ({ ...d }));
    },
    async createDevice(nd) {
      const d: Device = {
        id: nextId++,
        name: required("a name", nd.name),
        kind: nd.kind ?? "other",
        address: required("an address", nd.address),
        port: nd.port ?? 22,
        note: nd.note ?? "",
      };
      devices.push(d);
      return { ...d };
    },
    async updateDevice(id, p) {
      const d = device(id);
      if (p.name !== undefined) d.name = required("a name", p.name);
      if (p.kind) d.kind = p.kind;
      if (p.address !== undefined) d.address = required("an address", p.address);
      if (p.port !== undefined) d.port = p.port;
      if (p.note !== undefined) d.note = p.note;
      return { ...d };
    },
    async deleteDevice(id) {
      devices.splice(devices.indexOf(device(id)), 1);
    },
    async probeDevices() {
      return devices.map((d) => [d.id, !offline.includes(d.address)] as [number, boolean]);
    },
    // A demo run finishes on its first poll.
    async startRun(taskId, dir, access) {
      if (!config.server) throw new Error("No server set.");
      const t = task(taskId);
      const r: AgentRun = { id: nextId++, task_id: taskId, run: `${t.key.toLowerCase()}-${now()}`, dir, access, started: now(), ended: null, exit_code: null, tail: "" };
      runs.unshift(r);
      t.status = "in_progress";
      return { ...r };
    },
    async pollRun(run) {
      const r = findRun(run);
      if (r.ended === null) {
        Object.assign(r, { ended: now(), exit_code: 0, tail: "Pruned 4 images.\nFreed 2.1 GB." });
        task(r.task_id).status = "done";
      }
      return { ...r };
    },
    async stopRun(run) {
      const r = findRun(run);
      if (r.ended === null) {
        Object.assign(r, { ended: now(), exit_code: 143 });
        task(r.task_id).status = "todo";
      }
      return { ...r };
    },
    async runsFor(taskId) {
      return runs.filter((r) => r.task_id === taskId).map((r) => ({ ...r }));
    },
    async printers() {
      return printers.map((p) => ({ ...p, config: { ...p.config }, state: { ...p.state } }));
    },
    async addPrinter({ name, address, serial }) {
      if (!name.trim() || !address.trim() || !serial.trim()) throw new Error("A printer needs a name, an address and a serial.");
      if (printers.some((p) => p.config.serial === serial.trim())) throw new Error("That printer is already added.");
      printers.push({
        config: { name: name.trim(), address: address.trim(), serial: serial.trim(), cert_sha256: null },
        state: { gcode_state: null, percent: null, remaining_min: null, layer: null, total_layers: null, nozzle_c: null, nozzle_target_c: null, bed_c: null, bed_target_c: null, job: null },
        online: false,
        error: null,
      });
    },
    async removePrinter(serial) {
      printers.splice(printers.indexOf(printer(serial)), 1);
    },
    async printerCommand(serial, kind) {
      printer(serial);
      printerCommands.push([serial, kind]);
    },
    onPrinter() {
      return () => {};
    },
    async claudeHooks() {
      return config.claude_hooks;
    },
    async setClaudeHooks(on) {
      config = { ...config, claude_hooks: on };
      return on;
    },
    async repoStatus(projectId) {
      const p = project(projectId);
      return p.repo ? (seed.repos?.[projectId] ?? null) : null;
    },
  };
}

/** A made-up developer's workshop, for the preview and tests. */
/** A project made by hand, before its id and prefix. */
function hand(name: string): Omit<Project, "id" | "prefix" | "created"> {
  return {
    name,
    repo: null,
    note: null,
    state: "active",
    agent_dir: null,
    github: null,
    url: null,
    description: null,
    pushed_at: null,
    private: false,
    archived: false,
    color: colorFor(name),
    language: null,
  };
}

/** A project brought in from one of the made-up developer's repos. */
function fromRepo(r: Repo): Omit<Project, "id" | "prefix" | "created"> {
  return {
    ...hand(r.name),
    github: r.name_with_owner,
    url: r.url,
    description: r.description,
    language: r.language,
    pushed_at: r.pushed_at,
    private: r.is_private,
    archived: r.is_archived,
  };
}

/** A repo on the made-up developer's GitHub, pushed `days` ago. */
function repo(name: string, description: string, language: string, days: number): Repo {
  return {
    name,
    name_with_owner: `demo-dev/${name}`,
    description,
    url: `https://github.com/demo-dev/${name}`,
    is_private: false,
    is_archived: false,
    pushed_at: new Date(Date.now() - days * 86400000).toISOString(),
    language,
  };
}

function demoCloud(t: number): CloudSeed {
  const MB = 1024 ** 2;
  const dir = (rel: string, days: number) => ({ name: rel.split("/").pop()!, rel, kind: "dir" as const, size: 0, modified: t - days * 86400 });
  const file = (rel: string, size: number, days: number) => ({ ...dir(rel, days), kind: "file" as const, size });
  return {
    entries: [
      dir("Photos", 2),
      dir("Photos/2026", 2),
      file("Photos/2026/lake.jpg", 6 * MB, 2),
      file("Photos/2026/workshop.heic", 3 * MB, 9),
      dir("Documents", 5),
      file("Documents/lease.pdf", 2 * MB, 40),
      file("Documents/taxes-2025.xlsx", 0.4 * MB, 60),
      dir("Videos", 1),
      file("Videos/print-timelapse.mp4", 2300 * MB, 1),
      dir("Backups", 7),
      file("Backups/home-assistant.tar", 840 * MB, 7),
      file("Backups/old-site.zip", 12 * MB, 90),
      file("README.md", 0.002 * MB, 30),
      file(".DS_Store", 0.006 * MB, 1),
    ],
    zips: { "Backups/old-site.zip": [file("index.html", 0.01 * MB, 90)] },
  };
}

export function demoSeed(): Seed {
  const t = Math.floor(Date.now() / 1000);
  const day = localDay();
  const github: Repo[] = [
    repo("orbit", "A sync engine for local-first notes", "TypeScript", 1),
    { ...repo("dotfiles", "Shell, editor and terminal setup", "Shell", 12), is_private: true },
    repo("tidepool", "A tiny job queue on SQLite", "Rust", 6),
    repo("pixel-clock", "A tiny e-ink clock", "C++", 140),
    { ...repo("old-site", "The first personal site", "HTML", 400), is_archived: true },
    repo("homelab", "Compose files for the home server", "Dockerfile", 3),
    repo("weather-cli", "Forecasts in the terminal", "Go", 20),
  ];
  const [orbit, dotfiles, tidepool, pixel, oldSite] = github;
  const projects: Project[] = [
    {
      ...fromRepo(orbit),
      id: 1,
      prefix: "ORB",
      repo: "~/code/orbit",
      note: "/Volumes/vault/Projects/orbit.md",
      created: t - 864000,
    },
    { ...hand("homelab"), id: 2, prefix: "LAB", created: t - 432000, agent_dir: "~/homelab" },
    { ...fromRepo(dotfiles), id: 3, prefix: "DOT", created: t - 400000 },
    { ...fromRepo(tidepool), id: 4, prefix: "TID", created: t - 300000 },
    { ...fromRepo(pixel), id: 5, prefix: "PIX", created: t - 900000, state: "paused" },
    { ...fromRepo(oldSite), id: 6, prefix: "OLD", created: t - 990000, state: "paused" },
  ];
  const mk = (id: number, key: string, title: string, extra: Partial<Task>): Task => ({
    due_time: null,
    attention: false,
    sort: id,
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
    mk(1, "ORB-4", "Wire the sync status into the toolbar", {
      project_id: 1,
      status: "in_progress",
      priority: "high",
      notes: "Show the sync state beside the title, with the last error on hover.",
    }),
    mk(2, "LAB-2", "Update all containers and prune old images", {
      project_id: 2,
      assignee: "agent",
      priority: "medium",
      due: localDay(new Date(Date.now() + 86_400_000)),
      due_time: "09:00",
      notes: "Pull new images, restart one by one, then `docker image prune`. Skip **home-assistant** this week.",
    }),
    mk(3, "ORB-3", "Write release notes for 0.4", { project_id: 1, due: day }),
    mk(4, "TSK-1", "Print a new case lid for the Pi", { priority: "low" }),
    mk(5, "LAB-1", "Back up the Home Assistant config", { project_id: 2, assignee: "agent", status: "done", attention: true, notes: "Copied to the backup share. 840 MB." }),
    mk(6, "ORB-2", "Try a lighter font for the sidebar", { project_id: 1, status: "backlog" }),
    mk(7, "TSK-2", "Order PETG, black and white", { priority: "urgent" }),
    mk(8, "ORB-1", "Set up the project", { project_id: 1, status: "done", updated: t - 20 * 86400 }),
  ];
  const container = (name: string, image: string, up = true) => ({
    name,
    image,
    state: up ? "running" : "exited",
    status: up ? "Up 6 days" : "Exited (0) 3 hours ago",
  });
  const GB = 1024 ** 3;
  return {
    projects,
    tasks,
    done: {
      1: [{ date: day, time: "10:42", text: "Keep sync off by default until conflicts are handled." }],
      3: [{ date: day, time: "15:10", text: "Keep machine-specific settings in a local file that git ignores." }],
      2: [{ date: day, time: "09:05", text: "Move backups to the second disk and keep 14 days." }],
      4: [{ date: day, time: "12:30", text: "Store readings as integers in tenths of a degree." }],
    },
    devices: [
      { id: 101, name: "studio", kind: "mac", address: "10.0.7.10", port: 22, note: "" },
      { id: 102, name: "lab", kind: "server", address: "10.0.7.2", port: 22, note: "Raspberry Pi 5, 8 GB" },
      { id: 103, name: "printer", kind: "printer", address: "10.0.7.30", port: 8883, note: "" },
    ],
    server: {
      containers: [
        container("jellyfin", "jellyfin/jellyfin:latest"),
        container("pihole", "pihole/pihole:latest"),
        container("home-assistant", "ghcr.io/home-assistant/home-assistant:stable"),
        container("grafana", "grafana/grafana:11"),
        container("prometheus", "prom/prometheus:v3"),
        container("mosquitto", "eclipse-mosquitto:2"),
        container("caddy", "caddy:2"),
        container("uptime-kuma", "louislam/uptime-kuma:1"),
        container("syncthing", "syncthing/syncthing:latest", false),
      ],
      services: [
        { name: "cron", description: "Regular background program processing daemon" },
        { name: "docker", description: "Docker Application Container Engine" },
        { name: "smbd", description: "Samba SMB Daemon" },
        { name: "ssh", description: "OpenBSD Secure Shell server" },
        { name: "tailscaled", description: "Tailscale node agent" },
      ],
      load: [0.42, 0.37, 0.3],
      cores: 4,
      mem_total: 8 * GB,
      mem_available: 3.9 * GB,
      disk_size: 58 * GB,
      disk_used: 47 * GB,
      temp_c: 48.3,
      at: t,
    },
    printers: [
      {
        config: { name: "workshop", address: "10.0.7.30", serial: "DEMO0SERIAL01", cert_sha256: null },
        state: {
          gcode_state: "RUNNING",
          percent: 42,
          remaining_min: 73,
          layer: 120,
          total_layers: 300,
          nozzle_c: 219.8,
          nozzle_target_c: 220,
          bed_c: 54.9,
          bed_target_c: 55,
          job: "case-lid",
        },
        online: true,
        error: null,
      },
    ],
    github,
    cloud: demoCloud(t),
    repos: { 1: { branch: "main", changed: 3, ahead: 1, behind: 0, last_subject: "Fix the toolbar spacing", last_date: day } },
  };
}
