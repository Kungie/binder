import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Access,
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
  getConfig(): Promise<Config>;
  setConfig(config: Config): Promise<void>;
  vaultConnected(): Promise<boolean>;
  serverStatus(): Promise<ServerStatus>;
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
  getConfig: () => call("get_config"),
  setConfig: (config) => call("set_config", { config }),
  vaultConnected: () => call("vault_connected"),
  serverStatus: () => call("server_status"),
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
};

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Api = inTauri ? tauriApi : memoryApi(demoSeed());
