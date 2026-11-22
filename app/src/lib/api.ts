import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Friend, FriendsService, GhFilter, GhLists, GhDetail, GhAction, AgentReady, SiteCheck, Site, Brief, Uptime, PrintRecord,
  Access,
  Pane,
  AgentSession,
  ClaudeUsage,
  RecentRun,
  NewPrinter,
  PrinterCommand,
  PrinterView,
  AgentRun,
  AppChoice,
  CloudEntry,
  CloudInfo,
  CloudStatus,
  Config,
  Conflict,
  Device,
  ClaudeLimits,
  PingResult,
  PingSample,
  TailnetStatus,
  DevicePatch,
  GithubSync,
  HiddenRepo,
  DoneItem,
  DoneLine,
  NewDevice,
  NewProject,
  NewTask,
  Project,
  ProjectPatch,
  RepoStatus,
  ServerStatus,
  SyncReport,
  Task,
  TaskPatch,
  TaskView,
  Space,
  Transfer,
  TrashEntry,
  AppInfo,
  UpdateInfo,
  InstallProgress,
  TopProcess,
  TopFolder,
} from "./types";
import { demoSeed, memoryApi } from "./memory";

/** Everything the interface asks of the app. Tauri commands in the app, an in-memory fake elsewhere. */
export interface Api {
  listTasks(view: TaskView, includeClosed: boolean): Promise<Task[]>;
  createTask(task: NewTask): Promise<Task>;
  updateTask(id: number, patch: TaskPatch): Promise<Task>;
  deleteTask(id: number): Promise<void>;
  listProjects(): Promise<Project[]>;
  createProject(project: NewProject): Promise<Project>;
  updateProject(id: number, patch: ProjectPatch): Promise<Project>;
  deleteProject(id: number): Promise<void>;
  startProject(id: number): Promise<Project>;
  openNote(id: number): Promise<void>;
  logDone(projectId: number, text: string): Promise<boolean>;
  /** Newest first: daily files, then the note's old `## Done` lines. */
  readDone(projectId: number): Promise<DoneLine[]>;
  /** Today's lines across projects, newest first. */
  doneToday(): Promise<DoneItem[]>;
  /** When a done line was last written, Unix seconds. */
  lastDone(): Promise<number | null>;
  /** The morning brief: what happened since the last dismiss. */
  brief(): Promise<Brief>;
  dismissBrief(): Promise<void>;
  /** Something new for the brief was logged. */
  onBriefChanged(cb: () => void): () => void;
  /** The websites on the server, with their containers and figures. */
  sites(): Promise<Site[]>;
  /** Whether an https address answers within 5 s, and whether it may be framed. Asked from the Mac. */
  siteCheck(url: string): Promise<SiteCheck>;
  /** A site's page as a still snapshot, for sites that refuse to be framed. */
  siteSnapshot(url: string): Promise<string>;
  /** A site's last week: 28 six-hour bars, the share of good checks and the last answer time. */
  siteUptime(name: string): Promise<Uptime>;
  /** The quick add window made a task: it hides and the main window reloads. */
  quickAdded(): Promise<void>;
  /** Hides the quick add window. */
  quickClose(): Promise<void>;
  /** Calls back each time the quick add window is shown. Returns the unsubscribe. */
  onQuickOpen(cb: () => void): () => void;
  /** Saves the address of a site Binder found none for; returns it as kept (https added). */
  setSiteAddress(name: string, url: string): Promise<string>;
  /** Open pull requests and issues in the owner's projects, narrowed by a filter and maybe one project. */
  ghList(filter: GhFilter, projectId: number | null): Promise<GhLists>;
  ghDetail(repo: string, number: number, pull: boolean): Promise<GhDetail>;
  ghAct(repo: string, number: number, pull: boolean, action: GhAction): Promise<void>;
  /** Opens an issue in a project's repo; returns its number. */
  ghNewIssue(projectId: number, title: string, body: string): Promise<number>;
  ghAgentReady(projectId: number): Promise<AgentReady>;
  /** Starts the agent's review of a pull request; the draft comes from ghReviewDraft. */
  ghReview(projectId: number, repo: string, number: number): Promise<AgentRun>;
  /** The review a finished run wrote, or null while it runs. */
  ghReviewDraft(run: string): Promise<string | null>;
  ghWork(projectId: number, repo: string, number: number, title: string): Promise<AgentRun>;
  ghFix(projectId: number, repo: string, number: number): Promise<AgentRun>;
  /** Opens an https link in the browser. */
  openLink(url: string): Promise<void>;
  getConfig(): Promise<Config>;
  setConfig(config: Config): Promise<void>;
  vaultConnected(): Promise<boolean>;
  /** Mounts the vault share with the password saved in the Keychain; whether the vault is there after. */
  vaultConnect(): Promise<boolean>;
  serverStatus(): Promise<ServerStatus>;
  /** A container's last 200 log lines. */
  containerLogs(name: string): Promise<string>;
  restartContainer(name: string): Promise<void>;
  listDevices(): Promise<Device[]>;
  createDevice(device: NewDevice): Promise<Device>;
  updateDevice(id: number, patch: DevicePatch): Promise<Device>;
  deleteDevice(id: number): Promise<void>;
  /** Each device's id with whether it answered on its port. */
  probeDevices(): Promise<[number, boolean][]>;
  repoStatus(projectId: number): Promise<RepoStatus | null>;
  startRun(taskId: number, dir: string, access: Access): Promise<AgentRun>;
  pollRun(run: string): Promise<AgentRun>;
  stopRun(run: string): Promise<AgentRun>;
  /** A task's runs, newest first. */
  runsFor(taskId: number): Promise<AgentRun[]>;
  printers(): Promise<PrinterView[]>;
  /** The access code goes to the Keychain and is never sent back. */
  addPrinter(printer: NewPrinter): Promise<void>;
  removePrinter(serial: string): Promise<void>;
  printerCommand(serial: string, kind: PrinterCommand): Promise<void>;
  /** The last 50 prints, newest first. */
  printerHistory(): Promise<PrintRecord[]>;
  /** Calls back when a print ends, so the history reloads. Returns the unsubscribe. */
  onPrintEnded(cb: () => void): () => void;
  /** Streams the printer's camera, one JPEG per call. Returns the stop. */
  printerCamera(serial: string, onFrame: (jpeg: Uint8Array) => void): () => void;
  /** Calls back with each printer update. Returns the unsubscribe. */
  onPrinter(cb: (view: PrinterView) => void): () => void;
  /** Whether Binder's hooks are in ~/.claude/settings.json. */
  claudeHooks(): Promise<boolean>;
  setClaudeHooks(on: boolean): Promise<boolean>;
  /** Repos whose projects the owner deleted, newest first. */
  hiddenRepos(): Promise<HiddenRepo[]>;
  /** Lets the next sync bring a hidden repo back as a new project. */
  unhideRepo(nameWithOwner: string): Promise<void>;
  /** Opens a project's GitHub page in the browser. */
  openRepoPage(projectId: number): Promise<void>;
  /** Brings the owner's GitHub repos in as projects, through gh's own login. */
  githubSync(): Promise<SyncReport>;
  /** Calls back when a sync outside the interface (the one at start) finished. Returns the unsubscribe. */
  onProjectsChanged(cb: (sync: GithubSync) => void): () => void;
  cloudStatus(): Promise<CloudStatus>;
  cloudList(rel: string, showHidden: boolean): Promise<CloudEntry[]>;
  cloudMkdir(relDir: string, name: string): Promise<void>;
  cloudRename(rel: string, newName: string): Promise<void>;
  /** A move onto another disk runs as a transfer; its id comes back. */
  cloudMove(rel: string, destDirRel: string): Promise<number[]>;
  /** Moves to `.trash`; nothing in Cloud is deleted outright except by emptying the trash. */
  cloudTrash(rel: string): Promise<void>;
  cloudTrashList(): Promise<TrashEntry[]>;
  cloudRestore(id: string): Promise<void>;
  cloudEmptyTrash(): Promise<void>;
  /** A zip is extracted beside it instead, as a transfer. */
  cloudOpen(rel: string): Promise<number[]>;
  cloudCopy(rels: string[], destDirRel: string, conflict: Conflict): Promise<number[]>;
  cloudDuplicate(rels: string[]): Promise<number[]>;
  cloudCompress(rels: string[]): Promise<number[]>;
  cloudExtract(rel: string): Promise<number[]>;
  cloudInfo(rel: string): Promise<CloudInfo>;
  /** Bytes and item count. */
  cloudSize(rel: string): Promise<[number, number]>;
  /** The disk holding that folder. */
  cloudSpace(rel: string): Promise<Space>;
  cloudFastPath(): Promise<"ssh" | "smb">;
  /** Opens Quick Look, or closes it when open. */
  cloudQuickLook(rel: string): Promise<void>;
  cloudAppsFor(rel: string): Promise<AppChoice[]>;
  cloudOpenWith(rel: string, app: string): Promise<void>;
  /** Opens the share address so Finder mounts it. */
  cloudConnect(): Promise<void>;
  /** Returns the transfer ids; progress comes through `onTransfer`. */
  cloudUpload(paths: string[], destRel: string, conflict: Conflict): Promise<number[]>;
  cloudDownload(rels: string[], destDir: string, conflict: Conflict): Promise<number[]>;
  cloudCancel(id: number): Promise<void>;
  onTransfer(cb: (t: Transfer) => void): () => void;
  /** Files dropped on the window, with the pointer in CSS pixels. */
  onDrop(cb: (d: Dropped) => void): () => void;
  /** The pointer while files are dragged over the window; null when they leave. */
  onDragOver(cb: (p: { x: number; y: number } | null) => void): () => void;
  /** Starts a native drag of these Mac paths, out to Finder or back onto the window. */
  startDrag(paths: string[]): Promise<void>;
  /** Connects the pane's terminal, or hands a running one's output to `onData`. */
  termOpen(pane: Pane, cols: number, rows: number, onData: (bytes: Uint8Array) => void): Promise<void>;
  termWrite(session: string, data: string): Promise<void>;
  /** Ends a terminal here only. */
  termClose(session: string): Promise<void>;
  claudeLimits(): Promise<ClaudeLimits>;
  /** Adds Binder's status line to Claude Code on the server. */
  installClaudeLimits(): Promise<void>;
  /** Sends this Mac's traffic through an exit node, by its Tailscale address, or stops it with null. */
  vpnSet(exit: string | null): Promise<void>;
  /** Whether a server answers over SSH now. */
  serverReachable(target: string): Promise<boolean>;
  /** Binder VPN's sign-in service on the server. */
  friendsStatus(): Promise<FriendsService>;
  friendsList(): Promise<Friend[]>;
  friendAdd(name: string, password: string): Promise<void>;
  friendPassword(name: string, password: string): Promise<void>;
  /** Removes a friend and their devices from the tailnet. */
  friendRemove(name: string): Promise<void>;
  friendsOauth(id: string, secret: string): Promise<void>;
  tailnetStatus(): Promise<TailnetStatus>;
  /** Each device's pings of the last day, by Tailscale id. */
  tailnetHistory(): Promise<Record<string, PingSample[]>>;
  tailnetPing(ip: string): Promise<PingResult | null>;
  /** Sends files to a device with Taildrop. */
  taildrop(target: string, paths: string[]): Promise<void>;
  termResize(session: string, cols: number, rows: number): Promise<void>;
  /** Ends the local ssh and connects again; the tmux session on the server stays. */
  termReconnect(pane: Pane, cols: number, rows: number, onData: (bytes: Uint8Array) => void): Promise<void>;
  /** Binder's tmux sessions on the server and what each is doing. */
  agentSessions(): Promise<AgentSession[]>;
  /** The watcher's latest read of the sessions. */
  onAgentSessions(cb: (s: AgentSession[]) => void): () => void;
  /** Ends a session on the server and its terminal here. */
  closeAgentSession(session: string): Promise<void>;
  /** The session on screen, or null when the Agents screen is hidden. */
  agentViewing(session: string | null): Promise<void>;
  ollamaModels(): Promise<string[]>;
  /** Saves the model and ends the Ollama session, so the next connect runs it. */
  setOllamaModel(model: string): Promise<void>;
  claudeUsage(): Promise<ClaudeUsage>;
  recentRuns(): Promise<RecentRun[]>;
  /** Puts text on the Mac clipboard from the app, which works without a click. */
  copyText(text: string): Promise<void>;
  /** A terminal's ssh ended, with one line saying why. */
  onTermExit(cb: (session: string, message: string) => void): () => void;
  appInfo(): Promise<AppInfo>;
  /** The newer release, or null when Binder is up to date. */
  checkUpdate(): Promise<UpdateInfo | null>;
  /** Downloads, checks and installs the update, then the app restarts. */
  installUpdate(onProgress: (p: InstallProgress) => void): Promise<void>;
  /** Whether the crontab line for night tasks is on the server. */
  nightState(): Promise<boolean>;
  nightSetup(): Promise<boolean>;
  /** Empties the night queue, then takes the line out; returns the new state. */
  nightRemove(): Promise<boolean>;
  serverProcesses(byMemory: boolean): Promise<TopProcess[]>;
  serverFolders(): Promise<TopFolder[]>;
  /** Saves the interface size and scales the window to it. */
  setZoom(percent: number): Promise<void>;
  /** The app read or started agent runs in the background. */
  onTasksChanged(cb: () => void): () => void;
  /** A container alert was sent; the payload is its title. */
  onWatchAlert(cb: (title: string) => void): () => void;
}

export interface Dropped {
  paths: string[];
  x: number;
  y: number;
}

/** The picture under the pointer while dragging: a file, with a count for several. */
function dragIcon(count: number): string {
  const c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d");
  if (!g) return "";
  g.fillStyle = "#1C1C1F";
  g.strokeStyle = "#A1A1A1";
  g.lineWidth = 2;
  g.beginPath();
  g.roundRect(14, 6, 36, 48, 6);
  g.fill();
  g.stroke();
  if (count > 1) {
    g.fillStyle = "#EDEDED";
    g.beginPath();
    g.arc(48, 48, 12, 0, Math.PI * 2);
    g.fill();
    g.fillStyle = "#0B0B0C";
    g.font = "500 13px Inter, sans-serif";
    g.textAlign = "center";
    g.fillText(String(count), 48, 53);
  }
  return c.toDataURL("image/png");
}

function subscribe<T>(event: string, cb: (payload: T) => void): () => void {
  const off = listen<T>(event, (e) => cb(e.payload));
  return () => void off.then((f) => f());
}

/** A channel for a terminal's raw output bytes. */
function bytesChannel(onData: (bytes: Uint8Array) => void): Channel<ArrayBuffer | number[]> {
  const output = new Channel<ArrayBuffer | number[]>();
  output.onmessage = (m) => onData(m instanceof ArrayBuffer ? new Uint8Array(m) : Uint8Array.from(m));
  return output;
}

/** Tauri sends a command's error as a string; make it an Error so callers handle one shape. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(String(e));
  }
}

export const tauriApi: Api = {
  listTasks: (view, includeClosed) => call("list_tasks", { view, includeClosed }),
  createTask: (task) => call("create_task", { task }),
  updateTask: (id, patch) => call("update_task", { id, patch }),
  deleteTask: (id) => call("delete_task", { id }),
  listProjects: () => call("list_projects"),
  createProject: (project) => call("create_project", { project }),
  updateProject: (id, patch) => call("update_project", { id, patch }),
  deleteProject: (id) => call("delete_project", { id }),
  startProject: (id) => call("start_project", { id }),
  openNote: (id) => call("open_note", { id }),
  logDone: (projectId, text) => call("log_done", { projectId, text }),
  readDone: (projectId) => call("read_done", { projectId }),
  doneToday: () => call("done_today"),
  lastDone: () => call("last_done"),
  brief: () => call("brief"),
  dismissBrief: () => call("dismiss_brief"),
  sites: () => call("sites"),
  siteCheck: (url) => call("site_check", { url }),
  siteSnapshot: (url) => call("site_snapshot", { url }),
  siteUptime: (name) => call("site_uptime", { name }),
  quickAdded: () => call("quick_added"),
  quickClose: () => call("quick_close"),
  onQuickOpen: (cb) => subscribe<null>("quick://open", () => cb()),
  setSiteAddress: (name, url) => call("set_site_address", { name, url }),
  openLink: (url) => call("open_link", { url }),
  ghList: (filter, projectId) => call("gh_list", { filter, projectId }),
  ghDetail: (repo, number, pull) => call("gh_detail", { repo, number, pull }),
  ghAct: (repo, number, pull, action) => call("gh_act", { repo, number, pull, action }),
  ghNewIssue: (projectId, title, body) => call("gh_new_issue", { projectId, title, body }),
  ghAgentReady: (projectId) => call("gh_agent_ready", { projectId }),
  ghReview: (projectId, repo, number) => call("gh_review", { projectId, repo, number }),
  ghReviewDraft: (run) => call("gh_review_draft", { run }),
  ghWork: (projectId, repo, number, title) => call("gh_work", { projectId, repo, number, title }),
  ghFix: (projectId, repo, number) => call("gh_fix", { projectId, repo, number }),
  getConfig: () => call("get_config"),
  setConfig: (config) => call("set_config", { config }),
  vaultConnected: () => call("vault_connected"),
  vaultConnect: () => call("vault_connect"),
  serverStatus: () => call("server_status"),
  containerLogs: (name) => call("container_logs", { name }),
  restartContainer: (name) => call("restart_container", { name }),
  listDevices: () => call("list_devices"),
  createDevice: (device) => call("create_device", { device }),
  updateDevice: (id, patch) => call("update_device", { id, patch }),
  deleteDevice: (id) => call("delete_device", { id }),
  probeDevices: () => call("probe_devices"),
  repoStatus: (projectId) => call("repo_status", { projectId }),
  startRun: (taskId, dir, access) => call("start_run", { taskId, dir, access }),
  pollRun: (run) => call("poll_run", { run }),
  stopRun: (run) => call("stop_run", { run }),
  runsFor: (taskId) => call("runs_for", { taskId }),
  printers: () => call("printers"),
  addPrinter: ({ name, address, serial, accessCode }) => call("add_printer", { name, address, serial, accessCode }),
  removePrinter: (serial) => call("remove_printer", { serial }),
  printerCommand: (serial, kind) => call("printer_command", { serial, kind }),
  printerHistory: () => call("printer_history"),
  onPrintEnded: (cb) => subscribe<null>("printer://history", () => cb()),
  printerCamera: (serial, onFrame) => {
    const frames = bytesChannel(onFrame);
    void call("printer_camera", { serial, frames });
    return () => {
      frames.onmessage = () => {};
      void call("printer_camera_stop", { serial });
    };
  },
  claudeHooks: () => call("claude_hooks"),
  setClaudeHooks: (on) => call("set_claude_hooks", { on }),
  onPrinter: (cb) => subscribe<PrinterView>("printer://state", cb),
  openRepoPage: (projectId) => call("open_repo_page", { projectId }),
  hiddenRepos: () => call("hidden_repos"),
  unhideRepo: (nameWithOwner) => call("unhide_repo", { nameWithOwner }),
  githubSync: () => call("github_sync"),
  onProjectsChanged: (cb) => subscribe<GithubSync>("projects://changed", cb),
  cloudStatus: () => call("cloud_status"),
  cloudList: (rel, showHidden) => call("cloud_list", { rel, showHidden }),
  cloudMkdir: (relDir, name) => call("cloud_mkdir", { relDir, name }),
  cloudRename: (rel, newName) => call("cloud_rename", { rel, newName }),
  cloudMove: (rel, destDirRel) => call("cloud_move", { rel, destDirRel }),
  cloudTrash: (rel) => call("cloud_trash", { rel }),
  cloudTrashList: () => call("cloud_trash_list"),
  cloudRestore: (id) => call("cloud_restore", { id }),
  cloudEmptyTrash: () => call("cloud_empty_trash"),
  cloudOpen: (rel) => call("cloud_open", { rel }),
  cloudCopy: (rels, destDirRel, conflict) => call("cloud_copy", { rels, destDirRel, conflict }),
  cloudDuplicate: (rels) => call("cloud_duplicate", { rels }),
  cloudCompress: (rels) => call("cloud_compress", { rels }),
  cloudExtract: (rel) => call("cloud_extract", { rel }),
  cloudInfo: (rel) => call("cloud_info", { rel }),
  cloudSize: (rel) => call("cloud_size", { rel }),
  cloudSpace: (rel) => call("cloud_space", { rel }),
  cloudFastPath: () => call("cloud_fast_path"),
  cloudQuickLook: (rel) => call("cloud_quicklook", { rel }),
  cloudAppsFor: (rel) => call("cloud_apps_for", { rel }),
  cloudOpenWith: (rel, app) => call("cloud_open_with", { rel, app }),
  cloudConnect: () => call("cloud_connect"),
  cloudUpload: (paths, destRel, conflict) => call("cloud_upload", { paths, destRel, conflict }),
  cloudDownload: (rels, destDir, conflict) => call("cloud_download", { rels, destDir, conflict }),
  cloudCancel: (id) => call("cloud_cancel", { id }),
  onTransfer: (cb) => subscribe<Transfer>("cloud://transfer", cb),
  onDrop: (cb) => subscribe<Dropped>("cloud://drop", cb),
  onDragOver: (cb) => subscribe<{ x: number; y: number } | null>("cloud://drag-over", cb),
  startDrag: async (paths) => {
    const { startDrag } = await import("@crabnebula/tauri-plugin-drag");
    await startDrag({ item: paths, icon: dragIcon(paths.length) });
  },
  termOpen: (pane, cols, rows, onData) => call("term_open", { pane, cols, rows, output: bytesChannel(onData) }),
  termWrite: (session, data) => call("term_write", { session, data }),
  termClose: (session) => call("term_close", { session }),
  claudeLimits: () => call("claude_limits"),
  installClaudeLimits: () => call("install_claude_limits"),
  vpnSet: (exit) => call("vpn_set", { exit }),
  serverReachable: (target) => call("server_reachable", { target }),
  friendsStatus: () => call("friends_status"),
  friendsList: () => call("friends_list"),
  friendAdd: (name, password) => call("friend_add", { name, password }),
  friendPassword: (name, password) => call("friend_password", { name, password }),
  friendRemove: (name) => call("friend_remove", { name }),
  friendsOauth: (id, secret) => call("friends_oauth", { id, secret }),
  tailnetStatus: () => call("tailnet_status"),
  tailnetHistory: () => call("tailnet_history"),
  tailnetPing: (ip) => call("tailnet_ping", { ip }),
  taildrop: (target, paths) => call("taildrop", { target, paths }),
  termResize: (session, cols, rows) => call("term_resize", { session, cols, rows }),
  termReconnect: (pane, cols, rows, onData) => call("term_reconnect", { pane, cols, rows, output: bytesChannel(onData) }),
  agentSessions: () => call("agent_sessions"),
  onAgentSessions: (cb) => subscribe<AgentSession[]>("agents://sessions", cb),
  closeAgentSession: (session) => call("agent_session_close", { session }),
  agentViewing: (session) => call("agent_viewing", { session }),
  ollamaModels: () => call("ollama_models"),
  setOllamaModel: (model) => call("set_ollama_model", { model }),
  claudeUsage: () => call("claude_usage"),
  recentRuns: () => call("recent_runs"),
  copyText: (text) => call("copy_text", { text }),
  appInfo: () => call("app_info"),
  nightState: () => call("night_state"),
  nightSetup: () => call("night_setup"),
  nightRemove: () => call("night_remove"),
  serverProcesses: (byMemory) => call("server_processes", { byMemory }),
  serverFolders: () => call("server_folders"),
  setZoom: (percent) => call("set_zoom", { percent }),
  onTasksChanged: (cb) => subscribe<null>("tasks://changed", () => cb()),
  onBriefChanged: (cb) => subscribe<null>("brief://changed", () => cb()),
  onWatchAlert: (cb) => subscribe<string>("watch://alert", cb),
  checkUpdate: () => call("update_check"),
  installUpdate: (onProgress) => {
    const progress = new Channel<InstallProgress>();
    progress.onmessage = onProgress;
    return call("update_install", { progress });
  },
  onTermExit: (cb) => subscribe<{ session: string; message: string }>("term-exit", (e) => cb(e.session, e.message)),
};

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Api = inTauri ? tauriApi : memoryApi(demoSeed());
