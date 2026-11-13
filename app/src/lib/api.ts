import { invoke } from "@tauri-apps/api/core";
import type {
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
};

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api: Api = inTauri ? tauriApi : memoryApi(demoSeed());
