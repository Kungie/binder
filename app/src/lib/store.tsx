import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { Api } from "./api";
import type { AppInfo, GithubSync, InstallProgress, NewProject, UpdateInfo, NewTask, Project, ProjectPatch, Status, Task, TaskPatch, TaskView } from "./types";

export type Screen = "overview" | "tasks" | "projects" | "devices" | "server" | "agents" | "cloud" | "printer" | "settings";

export type UpdateState = { kind: "idle" } | { kind: "checking" } | { kind: "current" } | { kind: "available"; info: UpdateInfo } | { kind: "failed" };

/** An install kept here, not in the panel, so leaving Settings doesn't lose it. */
export type InstallState = { kind: "idle" } | { kind: "working"; progress: InstallProgress | null } | { kind: "failed" };

export interface Store {
  api: Api;
  tasks: Task[];
  projects: Project[];
  screen: Screen;
  setScreen(s: Screen): void;
  view: TaskView;
  setView(v: TaskView): void;
  selectedProject: number | null;
  openProject(id: number): void;
  creatingTask: boolean;
  /** Opens or closes New task; a status starts the task in that board column. */
  setCreatingTask(v: boolean, status?: Status): void;
  newTaskStatus: Status | null;
  creatingProject: boolean;
  setCreatingProject(v: boolean): void;
  reload(): Promise<void>;
  createTask(t: NewTask): Promise<Task>;
  updateTask(id: number, p: TaskPatch): Promise<Task>;
  deleteTask(id: number): Promise<void>;
  createProject(p: NewProject): Promise<Project>;
  updateProject(id: number, p: ProjectPatch): Promise<Project>;
  deleteProject(id: number): Promise<void>;
  logDone(projectId: number, text: string): Promise<boolean>;
  /** Bumps whenever a done line is written, so lists that read notes refetch. */
  doneVersion: number;
  /** The last GitHub sync seen this session, null before any. */
  github: GithubSync | null;
  syncing: boolean;
  syncGithub(): Promise<void>;
  /** Cloud items copied or cut, kept while the app runs. */
  cloudClipboard: CloudClipboard | null;
  setCloudClipboard(c: CloudClipboard | null): void;
  /** The running app; null until it answers. */
  app: AppInfo | null;
  update: UpdateState;
  /** Asks GitHub for a newer release. Quiet: a failure goes back to idle and shows nothing. */
  checkUpdate(quiet: boolean): Promise<void>;
  install: InstallState;
  /** Downloads and installs the update; the app restarts when it lands. */
  installUpdate(): Promise<void>;
  /** Whether a Cloud copy is in flight. */
  transferring: boolean;
}

export interface CloudClipboard {
  mode: "copy" | "cut";
  rels: string[];
}

const Ctx = createContext<Store | null>(null);

/** How long after the window opens Binder looks for an update. */
const UPDATE_DELAY = 5000;
/** A check that hasn't answered by then counts as failed. */
const CHECK_TIMEOUT = 30_000;

export function StoreProvider({
  api,
  children,
  initialScreen = "overview",
  updateDelay = UPDATE_DELAY,
  checkTimeout = CHECK_TIMEOUT,
}: {
  api: Api;
  children: ReactNode;
  initialScreen?: Screen;
  updateDelay?: number;
  checkTimeout?: number;
}) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [screen, setScreen] = useState<Screen>(initialScreen);
  const [view, setView] = useState<TaskView>({ kind: "all" });
  const [selectedProject, setSelectedProject] = useState<number | null>(null);
  const [creatingTask, setCreating] = useState(false);
  const [newTaskStatus, setNewTaskStatus] = useState<Status | null>(null);
  const setCreatingTask = useCallback((v: boolean, status?: Status) => {
    setCreating(v);
    setNewTaskStatus(v ? (status ?? null) : null);
  }, []);
  const [creatingProject, setCreatingProject] = useState(false);
  const [doneVersion, setDoneVersion] = useState(0);
  const [cloudClipboard, setCloudClipboard] = useState<CloudClipboard | null>(null);
  const [github, setGithub] = useState<GithubSync | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [app, setApp] = useState<AppInfo | null>(null);
  const [update, setUpdate] = useState<UpdateState>({ kind: "idle" });
  const [install, setInstall] = useState<InstallState>({ kind: "idle" });
  const [running, setRunning] = useState<ReadonlySet<number>>(new Set());
  const checked = useRef(false);

  const checkUpdate = useCallback(
    async (quiet: boolean) => {
      setUpdate({ kind: "checking" });
      try {
        const late = new Promise<never>((_, reject) => window.setTimeout(() => reject(new Error("No answer.")), checkTimeout));
        const info = await Promise.race([api.checkUpdate(), late]);
        setUpdate(info ? { kind: "available", info } : { kind: "current" });
      } catch {
        setUpdate(quiet ? { kind: "idle" } : { kind: "failed" });
      }
    },
    [api, checkTimeout],
  );

  const installUpdate = useCallback(async () => {
    setInstall({ kind: "working", progress: null });
    try {
      await api.installUpdate((progress) => setInstall({ kind: "working", progress }));
    } catch {
      setInstall({ kind: "failed" });
    }
  }, [api]);

  // Once, a few seconds after the window opens, and never in a development build.
  useEffect(() => {
    let timer: number | undefined;
    void api.appInfo().then((info) => {
      setApp(info);
      if (!info.updates || checked.current) return;
      checked.current = true;
      timer = window.setTimeout(() => void checkUpdate(true), updateDelay);
    });
    return () => window.clearTimeout(timer);
  }, [api, checkUpdate, updateDelay]);

  useEffect(
    () =>
      api.onTransfer((t) =>
        setRunning((s) => {
          if ((t.state === "running") === s.has(t.id)) return s;
          const next = new Set(s);
          if (t.state === "running") next.add(t.id);
          else next.delete(t.id);
          return next;
        }),
      ),
    [api],
  );

  const reload = useCallback(async () => {
    const [t, p] = await Promise.all([api.listTasks({ kind: "all" }, true), api.listProjects()]);
    setTasks(t);
    setProjects(p);
  }, [api]);

  useEffect(() => {
    void reload();
  }, [reload]);

  // The app reads and starts agent runs in the background; the board follows.
  useEffect(() => api.onTasksChanged(() => void reload()), [api, reload]);

  // The app syncs GitHub at start; projects reload when it lands.
  useEffect(
    () =>
      api.onProjectsChanged((sync) => {
        setGithub(sync);
        void reload();
      }),
    [api, reload],
  );

  const syncGithub = useCallback(async () => {
    setSyncing(true);
    try {
      await api.githubSync();
      setGithub({ at: Math.floor(Date.now() / 1000), error: null });
      await reload();
    } catch (e) {
      setGithub({ at: Math.floor(Date.now() / 1000), error: e instanceof Error ? e.message : String(e) });
    } finally {
      setSyncing(false);
    }
  }, [api, reload]);

  const after = useCallback(
    <T,>(run: Promise<T>) =>
      run.then(async (value) => {
        await reload();
        return value;
      }),
    [reload],
  );

  const store = useMemo<Store>(
    () => ({
      api,
      tasks,
      projects,
      screen,
      setScreen,
      view,
      setView,
      selectedProject,
      openProject(id) {
        setSelectedProject(id);
        setScreen("projects");
      },
      creatingTask,
      setCreatingTask,
      newTaskStatus,
      creatingProject,
      setCreatingProject,
      reload,
      createTask: (t) => after(api.createTask(t)),
      updateTask: (id, p) => after(api.updateTask(id, p)),
      deleteTask: (id) => after(api.deleteTask(id)),
      createProject: (p) => after(api.createProject(p)),
      updateProject: (id, p) => after(api.updateProject(id, p)),
      deleteProject: (id) => after(api.deleteProject(id)),
      logDone: (projectId, text) =>
        after(api.logDone(projectId, text)).then((written) => {
          setDoneVersion((v) => v + 1);
          return written;
        }),
      doneVersion,
      cloudClipboard,
      setCloudClipboard,
      github,
      syncing,
      syncGithub,
      app,
      update,
      checkUpdate,
      install,
      installUpdate,
      transferring: running.size > 0,
    }),
    [api, tasks, projects, screen, view, selectedProject, creatingTask, setCreatingTask, newTaskStatus, creatingProject, reload, after, doneVersion, cloudClipboard, github, syncing, syncGithub, app, update, checkUpdate, install, installUpdate, running],
  );

  return <Ctx.Provider value={store}>{children}</Ctx.Provider>;
}

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error("useStore needs a StoreProvider");
  return s;
}
