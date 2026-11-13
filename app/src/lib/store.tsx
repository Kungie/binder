import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { Api } from "./api";
import type { GithubSync, NewProject, NewTask, Project, ProjectPatch, Status, Task, TaskPatch, TaskView } from "./types";

export type Screen = "overview" | "tasks" | "projects" | "devices" | "server" | "printer" | "settings";

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
}

const Ctx = createContext<Store | null>(null);

export function StoreProvider({ api, children, initialScreen = "overview" }: { api: Api; children: ReactNode; initialScreen?: Screen }) {
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
  const [github, setGithub] = useState<GithubSync | null>(null);
  const [syncing, setSyncing] = useState(false);

  const reload = useCallback(async () => {
    const [t, p] = await Promise.all([api.listTasks({ kind: "all" }, true), api.listProjects()]);
    setTasks(t);
    setProjects(p);
  }, [api]);

  useEffect(() => {
    void reload();
  }, [reload]);

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
      github,
      syncing,
      syncGithub,
    }),
    [api, tasks, projects, screen, view, selectedProject, creatingTask, setCreatingTask, newTaskStatus, creatingProject, reload, after, doneVersion, github, syncing, syncGithub],
  );

  return <Ctx.Provider value={store}>{children}</Ctx.Provider>;
}

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error("useStore needs a StoreProvider");
  return s;
}
