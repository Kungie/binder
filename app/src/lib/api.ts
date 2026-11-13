import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Access,
  NewPrinter,
  PrinterCommand,
  PrinterView,
  AgentRun,
  Config,
  Device,
  DevicePatch,
  DoneLine,
  NewDevice,
  NewProject,
  NewTask,
  Project,
  ProjectPatch,
  RepoStatus,
  ServerStatus,
  Task,
  TaskPatch,
  TaskView,
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
  readDone(projectId: number): Promise<DoneLine[]>;
  doneToday(): Promise<[string, DoneLine][]>;
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
  onPrinter(cb) {
    const off = listen<PrinterView>("printer://state", (e) => cb(e.payload));
    return () => void off.then((f) => f());
  },
};

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Api = inTauri ? tauriApi : memoryApi(demoSeed());
