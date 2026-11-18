import { paneSession } from "./agents";
import { nextDue } from "./repeat";
import type { Api } from "./api";
import type { GhAction, PullRow, IssueRow, Site, BriefItem, AppInfo, Transfer, UpdateInfo, TopFolder, TopProcess, HiddenRepo, AgentRun, PrinterCommand, PrinterView, Config, Pane, AgentSession, RecentRun, ClaudeLimits, TailnetStatus, PingSample, Device, DoneItem, DoneLine, NewProject, Project, Repo, RepoStatus, ServerStatus, SyncReport, Task } from "./types";
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
  /** Claude Code's limits; not installed when unset. */
  limits?: ClaudeLimits;
  /** The tailnet; null when Tailscale isn't installed. A demo tailnet when unset. */
  tailnet?: TailnetStatus | null;
  /** Whether the Cloud share is mounted. Defaults to true. */
  cloudMounted?: boolean;
  vaultConnected?: boolean;
  /** Whether Settings › Claude Code starts on. */
  claudeHooks?: boolean;
  /** The SSH target in the config. Defaults to "lab"; null means none is set. */
  server?: string | null;
  /** Device addresses that don't answer a probe. */
  offline?: string[];
  /** When set, the agents' terminals fail to connect with this line. */
  termFail?: string;
  /** The tmux sessions on the server; Claude Code and Ollama, idle, when unset. */
  sessions?: AgentSession[];
  /** The newer release the update check finds. Defaults to none. */
  update?: UpdateInfo | null;
  /** Makes the update check or the install fail. */
  updateFail?: "check" | "install";
  /** A development build, which never updates. */
  dev?: boolean;
  /** The update check never answers. */
  updateHang?: boolean;
  /** Awaited after each install step, so a test can look in between. */
  installGate?: () => Promise<void>;
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
  cameraStops: string[];
  /** Sessions ended from the Agents screen, in order. */
  closedSessions: string[];
  /** Files sent with Taildrop. */
  dropped: { target: string; paths: string[] }[];
  /** Containers restarted, in order. */
  restarted: string[];
  /** Links opened in the browser. */
  openedLinks: string[];
  /** GitHub actions taken: repo, number, pull, action. */
  ghActions: [string, number, boolean, GhAction][];
  /** Issues opened: repo, title, body. */
  ghNewIssues: [string, string, string][];
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
  /** Sends a Cloud transfer event. */
  emitTransfer(t: Transfer): void;
  /** Whether an update was installed. */
  readonly installed: boolean;
  /** Acts as if the app read or started agent runs in the background. */
  emitTasksChanged(): void;
  /** Acts as if a container alert was sent. */
  emitWatchAlert(title: string): void;
} {
  const tasks = seed.tasks.map((t) => ({ ...t }));
  const projects = seed.projects.map((p) => ({ ...p }));
  const done: Record<number, DoneLine[]> = Object.fromEntries(Object.entries(seed.done).map(([k, v]) => [k, [...v]]));
  const counters: Record<string, number> = {};
  const devices = (seed.devices ?? []).map((d) => ({ ...d }));
  const runs: AgentRun[] = [];
  const printers: PrinterView[] = (seed.printers ?? []).map((p) => ({ ...p, config: { ...p.config }, state: { ...p.state } }));
  const printerCommands: [string, PrinterCommand][] = [];
  const cameraStops: string[] = [];
  const restarted: string[] = [];
  const openedLinks: string[] = [];
  const ghActions: [string, number, boolean, GhAction][] = [];
  const ghNewIssues: [string, string, string][] = [];
  const hoursAgo = (h: number) => new Date(Date.now() - h * 3_600_000).toISOString();
  const ghPulls: PullRow[] = [
    { repo: "demo-dev/orbit", number: 41, title: "Show sync status as a dot", author: "friend", ci: "passing", review: "review_required", review_requested: true, mine: false, updated: hoursAgo(1) },
    { repo: "demo-dev/orbit", number: 40, title: "Start faster with a lazy store", author: "demo-dev", ci: "failing", review: null, review_requested: false, mine: true, updated: hoursAgo(3) },
    { repo: "demo-dev/tidepool", number: 39, title: "Bump the chart library", author: "dependabot", ci: "passing", review: null, review_requested: false, mine: false, updated: hoursAgo(20) },
  ];
  const ghIssues: IssueRow[] = [
    { repo: "demo-dev/orbit", number: 12, title: "Crash on start with an empty vault", author: "friend", assignees: ["demo-dev"], updated: hoursAgo(2) },
    { repo: "demo-dev/tidepool", number: 7, title: "Typo in the readme", author: "friend", assignees: [], updated: hoursAgo(30) },
  ];
  const ghRepos = () => new Set(projects.map((p) => p.github).filter(Boolean) as string[]);
  const ghProject = (id: number) => {
    const p = project(id);
    if (!p.github) throw new Error("This project isn't on GitHub.");
    return p;
  };
  const DRAFT = "It adds a sync dot to the toolbar.\n\nRisks: the dot never turns grey when the sync stops, so a dead sync looks healthy.\n\nFix: in `toolbar.ts:42`, clear the state on disconnect.";
  const MB = 1024 * 1024;
  const sites: Site[] = [
    { name: "blog", address: "https://blog.example.com", containers: [{ name: "blog-site-1", state: "running" }, { name: "blog-cloudflared-1", state: "running" }], cpu: 0.4, mem_bytes: 38 * MB, disk_bytes: 12 * MB },
    { name: "shop", address: "https://shop.example.com", containers: [{ name: "shop-site-1", state: "exited" }], cpu: 0, mem_bytes: 0, disk_bytes: 240 * MB },
    { name: "notes", address: null, containers: [{ name: "notes-site-1", state: "running" }], cpu: 1.2, mem_bytes: 1536 * MB, disk_bytes: 3 * MB },
  ];
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
  const terms = new Map<string, (bytes: Uint8Array) => void>();
  const sessions: AgentSession[] = options.sessions ?? [
    { session: "binder-claude", state: "idle", line: "? for shortcuts" },
    { session: "binder-ollama", state: "idle", line: ">>> Send a message" },
  ];
  const closedSessions: string[] = [];
  const dropped: { target: string; paths: string[] }[] = [];
  const limits: ClaudeLimits = options.limits ?? { installed: false, limits: null };
  const tailnet: TailnetStatus | null =
    options.tailnet === undefined
      ? {
          running: true,
          peers: [
            { id: "n1", name: "demo-mac", dns: "demo-mac.tail0.ts.net", os: "macOS", ip: "100.64.0.1", online: true, last_seen: null, key_expiry: null, expired: false, relay: "fra", taildrop: false, this_mac: true },
            { id: "n2", name: "lab", dns: "lab.tail0.ts.net", os: "linux", ip: "100.64.0.2", online: true, last_seen: null, key_expiry: null, expired: false, relay: "fra", taildrop: true, this_mac: false },
            { id: "n3", name: "phone", dns: "phone.tail0.ts.net", os: "iOS", ip: "100.64.0.3", online: false, last_seen: Math.floor(Date.now() / 1000) - 3 * 86_400, key_expiry: null, expired: true, relay: "", taildrop: true, this_mac: false },
          ],
        }
      : options.tailnet;
  const encoder = new TextEncoder();
  const copied: string[] = [];
  let installed = false;
  let night = false;
  const tasksChanged = new Set<() => void>();
  const alerts = new Set<(title: string) => void>();
  const processes: TopProcess[] = [
    { name: "ollama", cpu: 41.2, mem: 12.5, rss_mb: 1024 },
    { name: "node", cpu: 2, mem: 3.3, rss_mb: 264 },
    { name: "caddy", cpu: 0.4, mem: 0.9, rss_mb: 72 },
  ];
  const folders: TopFolder[] = [
    { path: "/srv/cloud", bytes: 590_000_000_000 },
    { path: "/home/demo-dev", bytes: 150_000_000_000 },
    { path: "/srv/cloud/SD/photos", bytes: 70_000_000_000 },
  ];
  async function openTerm(pane: Pane, onData: (bytes: Uint8Array) => void) {
    if (!config.server) throw new Error("No server set.");
    if (options.termFail) throw new Error(options.termFail);
    const agent = paneSession(pane);
    if (!sessions.some((s) => s.session === agent)) sessions.push({ session: agent, state: "idle", line: "" });
    terms.set(agent, onData);
    onData(encoder.encode("Connected to demo-dev@10.0.7.2 (demo)\r\n"));
  }
  let config: Config = { vault: "/Volumes/vault", server: options.server === undefined ? "lab" : options.server, printers: [], claude_hooks: options.claudeHooks ?? false, cloud: "/Volumes/cloud", cloud_share: null, cloud_remote: "/srv/cloud", pick_up: true, watch: true, zoom: 100, print_notify: true, site_addresses: {}, overview: { left: ["numbers", "done_today"], right: ["server", "cloud", "printer", "open_tasks"], split: 50, heights: {} }, ollama_model: "qwen3:4b" };
  let lastDone: number | null = seed.lastDone ?? null;
  let briefDismissed = false;
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
  let vaultUp = options.vaultConnected ?? true;
  const connected = () => vaultUp;

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
    termOpen: (agent, _cols, _rows, onData) => openTerm(agent, onData),
    async termWrite(agent, data) {
      terms.get(agent)?.(encoder.encode(data));
    },
    async claudeLimits() {
      return structuredClone(limits);
    },
    async installClaudeLimits() {
      limits.installed = true;
    },
    async termClose(session) {
      terms.delete(session);
    },
    async tailnetStatus() {
      if (tailnet === null) throw new Error("Tailscale isn't installed on this Mac.");
      return structuredClone(tailnet);
    },
    async tailnetHistory() {
      const now = Math.floor(Date.now() / 1000);
      const lab: PingSample[] = Array.from({ length: 24 }, (_, i) => ({ at: now - (23 - i) * 3600, ms: 28 + ((i * 7) % 11), via: "DERP fra" }));
      return { n2: lab };
    },
    async tailnetPing(ip) {
      const p = tailnet?.peers.find((x) => x.ip === ip);
      return p?.online ? { ms: 31, via: "DERP fra" } : null;
    },
    async taildrop(target, paths) {
      dropped.push({ target, paths });
    },
    async termResize() {},
    termReconnect: (agent, _cols, _rows, onData) => openTerm(agent, onData),
    async copyText(text) {
      copied.push(text);
    },
    async agentSessions() {
      if (!config.server) throw new Error("No server set.");
      return sessions.map((s) => ({ ...s }));
    },
    onAgentSessions() {
      return () => {};
    },
    async closeAgentSession(session) {
      closedSessions.push(session);
      terms.delete(session);
      const i = sessions.findIndex((s) => s.session === session);
      if (i >= 0) sessions.splice(i, 1);
    },
    async agentViewing() {},
    async ollamaModels() {
      return ["qwen3:4b", "llama3.2:latest"];
    },
    async setOllamaModel(model) {
      if (!/^[A-Za-z0-9._:/-]+$/.test(model)) throw new Error("That isn't a model name.");
      config = { ...config, ollama_model: model };
      terms.delete("binder-ollama");
    },
    async claudeUsage() {
      return { five_hours: 182_000, today: 1_240_000, week: 8_900_000, sessions_today: 3, days: [900_000, 2_100_000, 0, 1_500_000, 1_800_000, 1_360_000, 1_240_000] };
    },
    async recentRuns(): Promise<RecentRun[]> {
      return runs.filter((r) => r.ended === null || r.ended > now() - 86_400).map((r) => ({ run: { ...r }, key: task(r.task_id).key, title: task(r.task_id).title }));
    },
    onTermExit() {
      return () => {};
    },
    async appInfo(): Promise<AppInfo> {
      return { version: "2.4.0", updates: !options.dev };
    },
    async checkUpdate() {
      if (options.updateFail === "check") throw new Error("Network is unreachable.");
      if (options.updateHang) return new Promise<never>(() => {});
      return options.update ?? null;
    },
    async installUpdate(onProgress) {
      const gate = options.installGate ?? (async () => {});
      onProgress({ kind: "downloading", percent: 42 });
      await gate();
      if (options.updateFail === "install") throw new Error("The signature doesn't match.");
      onProgress({ kind: "downloading", percent: 100 });
      onProgress({ kind: "installing" });
      await gate();
      installed = true;
    },
    get installed() {
      return installed;
    },
    async nightState() {
      if (!config.server) throw new Error("No server set.");
      return night;
    },
    async nightSetup() {
      night = true;
      return night;
    },
    async nightRemove() {
      night = false;
      return night;
    },
    async serverProcesses(byMemory) {
      return [...processes].sort((a, b) => (byMemory ? b.rss_mb - a.rss_mb : b.cpu - a.cpu));
    },
    async serverFolders() {
      return folders.map((f) => ({ ...f }));
    },
    async setZoom(percent) {
      if (![100, 110, 125, 150].includes(percent)) throw new Error("The interface size is 100, 110, 125 or 150%.");
      config = { ...config, zoom: percent };
    },
    onBriefChanged() {
      return () => {};
    },
    onTasksChanged(cb) {
      tasksChanged.add(cb);
      return () => void tasksChanged.delete(cb);
    },
    onWatchAlert(cb) {
      alerts.add(cb);
      return () => void alerts.delete(cb);
    },
    emitTasksChanged() {
      tasksChanged.forEach((cb) => cb());
    },
    emitWatchAlert(title) {
      alerts.forEach((cb) => cb(title));
    },
    done,
    printerCommands,
    cameraStops,
    closedSessions,
    dropped,
    restarted,
    openedLinks,
    ghActions,
    ghNewIssues,
    async ghList(filter, projectId) {
      const repos = ghRepos();
      const only = projectId === null ? null : project(projectId).github;
      const keep = (r: { repo: string }) => repos.has(r.repo) && (only === null || r.repo === only);
      let pulls = ghPulls.filter(keep);
      let issues = ghIssues.filter(keep);
      if (filter === "needs_you") {
        pulls = pulls.filter((p) => p.review_requested || (p.mine && (p.ci === "failing" || p.review === "changes_requested")));
        issues = issues.filter((i) => i.assignees.includes("demo-dev"));
      } else if (filter === "mine") {
        pulls = pulls.filter((p) => p.mine);
        issues = issues.filter((i) => i.author === "demo-dev" || i.assignees.includes("demo-dev"));
      }
      return { login: "demo-dev", pulls: pulls.map((p) => ({ ...p })), issues: issues.map((i) => ({ ...i, assignees: [...i.assignees] })) };
    },
    async ghDetail(repo, number, pull) {
      const url = `https://github.com/${repo}/${pull ? "pull" : "issues"}/${number}`;
      if (!pull) return { title: "Crash on start with an empty vault", body: "Open Orbit with an empty vault folder and it quits at once.", head: null, base: null, additions: 0, deletions: 0, files: 0, checks: [], comments: [], url };
      return {
        title: "Show sync status as a dot",
        body: "Shows whether sync is running as a small dot in the toolbar.",
        head: "feature/sync-dot",
        base: "main",
        additions: 120,
        deletions: 34,
        files: 6,
        checks: [["test", "passing"], ["lint", "passing"]],
        comments: [{ author: "friend", body: "Looks fine to me.", at: hoursAgo(1), state: null }],
        url,
      };
    },
    async ghAct(repo, number, pull, action) {
      ghActions.push([repo, number, pull, action]);
    },
    async ghNewIssue(projectId, title, body) {
      const p = ghProject(projectId);
      if (!title.trim()) throw new Error("An issue needs a title.");
      ghNewIssues.push([p.github!, title, body]);
      return 57;
    },
    async ghAgentReady(projectId) {
      return { gh: true, folder: !!project(projectId).agent_dir };
    },
    async ghReview(projectId, _repo, number) {
      const p = ghProject(projectId);
      if (!p.agent_dir) throw new Error("Set the project's agent folder first.");
      const t = await this.createTask({ title: `Review #${number}: pull request`, project_id: p.id, assignee: "agent", status: "in_progress" });
      return this.startRun(t.id, p.agent_dir, "read_only");
    },
    async ghReviewDraft() {
      return DRAFT;
    },
    async ghWork(projectId, _repo, _number, title) {
      const p = ghProject(projectId);
      if (!p.agent_dir) throw new Error("Set the project's agent folder first.");
      const t = await this.createTask({ title, project_id: p.id, assignee: "agent" });
      return this.startRun(t.id, p.agent_dir, "full");
    },
    async ghFix(projectId, _repo, number) {
      const p = ghProject(projectId);
      if (!p.agent_dir) throw new Error("Set the project's agent folder first.");
      const t = await this.createTask({ title: `Fix #${number}`, project_id: p.id, assignee: "agent" });
      return this.startRun(t.id, p.agent_dir, "full");
    },
    async sites() {
      if (!config.server) throw new Error("No server set.");
      return sites.map((s) => ({ ...s, address: config.site_addresses[s.name] ?? s.address, containers: s.containers.map((c) => ({ ...c })) }));
    },
    async siteCheck(url) {
      if (!url.startsWith("https://")) throw new Error("Only https addresses are checked.");
      return { status: 200, frameable: !url.includes("shop."), error: null };
    },
    async siteUptime(name) {
      const buckets = Array.from({ length: 28 }, (_, i) => (name === "shop" ? i !== 20 : true));
      return { buckets, percent: name === "shop" ? 96.4 : 99.7, ms: 120 };
    },
    async quickAdded() {
      tasksChanged.forEach((cb) => cb());
    },
    async quickClose() {},
    onQuickOpen() {
      return () => {};
    },
    async siteSnapshot(url) {
      if (!url.startsWith("https://")) throw new Error("Only https addresses are read.");
      return `<base href="${url}/"><body style="margin:0;font:48px sans-serif;background:#f4f1ea;color:#222;display:grid;place-items:center;height:100vh">Shop</body>`;
    },
    async setSiteAddress(name, url) {
      const kept = url.trim().startsWith("https://") ? url.trim() : `https://${url.trim().replace(/^http:\/\//, "")}`;
      config = { ...config, site_addresses: { ...config.site_addresses, [name]: kept } };
      return kept;
    },
    async openLink(url) {
      if (!url.startsWith("https://")) throw new Error("Only https links open.");
      openedLinks.push(url);
    },
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
        assignee: t.night ? "agent" : (t.assignee ?? "me"),
        due: t.due ?? null,
        due_time: t.due ? (t.due_time ?? null) : null,
        created: now(),
        updated: now(),
        attention: false,
        sort,
        night: t.night ?? false,
        closed: status === "done" || status === "canceled" ? now() : null,
        issue_number: null,
        issue_url: null,
        repeat: t.repeat ?? null,
      };
      tasks.push(created);
      return { ...created };
    },
    async updateTask(id, p) {
      const t = task(id);
      const finished = p.status === "done" && t.status !== "done";
      if (p.title !== undefined) {
        if (!p.title.trim()) throw new Error("A task needs a title.");
        t.title = p.title.trim();
      }
      if (p.notes !== undefined) t.notes = p.notes;
      if (p.status) {
        const closed = (s: string) => s === "done" || s === "canceled";
        if (closed(p.status) && !closed(t.status)) t.closed = now();
        else if (!closed(p.status)) t.closed = null;
        t.status = p.status;
      }
      if (p.priority) t.priority = p.priority;
      if (p.clear_project) t.project_id = null;
      else if (p.project_id !== undefined) t.project_id = project(p.project_id).id;
      if (p.assignee) {
        t.assignee = p.assignee;
        if (p.assignee === "me") t.night = false;
      }
      if (p.clear_due) (t.due = null), (t.due_time = null);
      else if (p.due !== undefined) t.due = p.due;
      if (p.clear_due_time) t.due_time = null;
      else if (p.due_time !== undefined) {
        if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(p.due_time)) throw new Error("A time is HH:MM.");
        t.due_time = p.due_time;
      }
      if (p.attention !== undefined) t.attention = p.attention;
      if (p.sort !== undefined) t.sort = p.sort;
      if (p.night !== undefined) {
        t.night = p.night;
        if (p.night) t.assignee = "agent";
      }
      if (p.clear_repeat) t.repeat = null;
      else if (p.repeat) t.repeat = p.repeat;
      t.updated = Math.max(now(), t.updated + 1);
      // A finished repeating task hands its rule to the next one.
      const rule = finished ? t.repeat : null;
      if (rule) {
        t.repeat = null;
        const { title, notes, priority, project_id, assignee, due_time, night } = t;
        await this.createTask({
          title,
          notes,
          priority,
          project_id: project_id ?? undefined,
          assignee,
          due: nextDue(rule, t.due, localDay()),
          due_time: due_time ?? undefined,
          night,
          repeat: rule,
        });
      }
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
        open_prs: null,
        ci: null,
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
    async brief() {
      if (briefDismissed) return { since: now(), shown: false, items: [] };
      const night = tasks.find((x) => x.assignee === "agent") ?? tasks[0];
      const issue = tasks.find((x) => x !== night) ?? tasks[0];
      const t = now();
      const items: BriefItem[] = [
        { kind: "server_restart", at: t - 6 * 3600 },
        { kind: "container", name: "web", problem: "stopped", at: t - 5 * 3600, back_at: t - 5 * 3600 + 360 },
        { kind: "run", task_id: night.id, key: night.key, ok: true, night: true, at: t - 4 * 3600 },
        { kind: "print", job: "case-lid", ok: true, at: t - 3 * 3600 },
        { kind: "issue", task_id: issue.id, key: issue.key, title: issue.title, at: t - 2 * 3600 },
      ];
      return { since: t - 86_400, shown: true, items };
    },
    async dismissBrief() {
      briefDismissed = true;
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
    async vaultConnect() {
      vaultUp = true;
      return true;
    },
    async containerLogs(name) {
      if (!reachable) throw new Error(`Can't reach ${config.server}`);
      return [`${name} started`, "Listening on :8080", "GET / 200 3ms"].join("\n");
    },
    async restartContainer(name) {
      if (!reachable) throw new Error(`Can't reach ${config.server}`);
      restarted.push(name);
    },
    async serverStatus() {
      if (!config.server) throw new Error("No server set.");
      if (!reachable || !seed.server) throw new Error(`Can't reach ${config.server}`);
      return { ...seed.server, hostname: config.server === "lab" ? "citadel" : config.server.split("@").pop()!, at: now() };
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
      const r: AgentRun = { id: nextId++, task_id: taskId, run: `${t.key.toLowerCase()}-${now()}`, dir, access, started: now(), ended: null, exit_code: null, tail: "", queued: false };
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
        Object.assign(r, { ended: now(), exit_code: 143, queued: false });
        Object.assign(task(r.task_id), { status: "todo", night: false });
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
    async printerHistory() {
      const t = now();
      return [
        { at: t - 3 * 3600, job: "bracket", ok: false, minutes: null, layer: 12 },
        { at: t - 26 * 3600, job: "case-lid", ok: true, minutes: 95, layer: null },
      ];
    },
    onPrintEnded() {
      return () => {};
    },
    printerCamera(serial, onFrame) {
      printer(serial);
      const timer = setTimeout(() => onFrame(new Uint8Array([0xff, 0xd8, 0xff, 0xd9])), 10);
      return () => {
        clearTimeout(timer);
        cameraStops.push(serial);
      };
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
    open_prs: null,
    ci: null,
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
      open_prs: 2,
      ci: "failing",
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
    night: false,
    closed: null,
    issue_number: null,
    issue_url: null,
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
    mk(6, "ORB-2", "Try a lighter font for the sidebar", { project_id: 1, status: "backlog", issue_number: 12, issue_url: "https://github.com/demo-dev/orbit/issues/12" }),
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
      power_w: 4.5,
      sd_size: 127_000_000_000,
      sd_used: 31_000_000_000,
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
          light_on: true,
          fan_percent: 60,
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
