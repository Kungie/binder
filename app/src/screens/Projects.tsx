import { useEffect, useState } from "react";
import { Panel } from "../components/Panel";
import { TaskBoard } from "../components/TaskBoard";
import { Button, Notice } from "../components/ui";
import { pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { viewTasks } from "../lib/tasks";
import { type DoneLine, type Project, type ProjectState, type RepoStatus, isClosed } from "../lib/types";
import { useVault, vaultNotice } from "../lib/useVault";

function DoneLog({ project }: { project: Project }) {
  const { api, logDone, doneVersion } = useStore();
  const [lines, setLines] = useState<DoneLine[]>([]);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void api.readDone(project.id).then((l) => live && setLines([...l].reverse()));
    return () => {
      live = false;
    };
  }, [api, project.id, project.note, doneVersion]);

  const add = async () => {
    try {
      if (await logDone(project.id, text)) setText("");
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <Panel title="Done" action={<span className="font-mono text-[11px] text-text-3">{lines.length}</span>}>
      <div className="border-t border-line px-3 py-2">
        <input
          aria-label="Add a done line"
          placeholder="A decision, in one line"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void add()}
          className="h-7 w-full bg-transparent text-[12px] placeholder:text-text-3 focus:outline-none"
        />
        {error && <Notice>{error.startsWith("Vault not connected") ? "Vault not connected" : error}</Notice>}
      </div>
      <ul aria-label="Done log" className="max-h-[320px] overflow-y-auto">
        {lines.map((l, i) => (
          <li key={`${l.date}-${i}`} className="grid grid-cols-[76px_minmax(0,1fr)] gap-3 border-t border-line px-3 py-2 text-[12px]">
            <span className="font-mono text-[11px] text-text-3">{l.date}</span>
            <span className="text-text-2">{l.text}</span>
          </li>
        ))}
        {lines.length === 0 && <li className="border-t border-line px-3 py-3 text-[12px] text-text-3">Nothing logged yet.</li>}
      </ul>
    </Panel>
  );
}

function GitRow({ project }: { project: Project }) {
  const { api } = useStore();
  const [repo, setRepo] = useState<RepoStatus | null>(null);
  useEffect(() => {
    let live = true;
    setRepo(null);
    if (project.repo) void api.repoStatus(project.id).then((r) => live && setRepo(r), () => live && setRepo(null));
    return () => {
      live = false;
    };
  }, [api, project.id, project.repo]);
  if (!repo) return null;
  return (
    <div role="group" aria-label="Git" className="flex min-w-0 items-center gap-3 text-[12px]">
      <span className="w-20 shrink-0 text-text-3">Git</span>
      <span className="font-mono text-[11px] text-text">{repo.branch ?? "detached"}</span>
      <span className="text-text-2">{repo.changed ? `${repo.changed} changed` : "Clean"}</span>
      <span className="font-mono text-[11px] text-text-2">{`↑${repo.ahead} ↓${repo.behind}`}</span>
      {repo.last_subject && <span className="min-w-0 truncate text-text-2">{repo.last_subject}</span>}
      {repo.last_date && <span className="shrink-0 font-mono text-[11px] text-text-3">{repo.last_date}</span>}
    </div>
  );
}

function Detail({ project }: { project: Project }) {
  const { api, tasks, updateProject, deleteProject, reload } = useStore();
  const vault = useVault();
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const mine = viewTasks(tasks, { kind: "project", id: project.id }, false);

  const startOrOpen = async () => {
    setError(null);
    try {
      if (project.note) await api.openNote(project.id);
      else await api.startProject(project.id);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      vault.recheck();
    }
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col gap-4 overflow-y-auto p-5">
      <header className="flex items-center gap-3">
        <h1 className="text-[15px] font-medium">{project.name}</h1>
        <span className="font-mono text-[11px] text-text-3">{project.prefix}</span>
        <select
          aria-label="State"
          value={project.state}
          onChange={(e) => void updateProject(project.id, { state: e.target.value as ProjectState })}
          className="h-6 rounded-control border border-line bg-bg px-1.5 text-[12px] text-text-2"
        >
          <option value="active">Active</option>
          <option value="paused">Paused</option>
          <option value="done">Done</option>
        </select>
        <Button variant="primary" className="ml-auto" onClick={() => void startOrOpen()}>
          {project.note ? "Open note" : "Start project"}
        </Button>
      </header>
      {(vault.connected === false || error) && <Notice>{error && !error.startsWith("Vault not connected") ? error : vaultNotice(vault.path)}</Notice>}
      <div className="flex items-center gap-2 text-[12px]">
        <span className="w-20 text-text-3">Repo</span>
        <span className="truncate font-mono text-[11px] text-text-2">{project.repo ?? "None"}</span>
        <Button variant="ghost" onClick={() => void pickFolder().then((f) => (f ? updateProject(project.id, { repo: f }) : null))}>
          Choose
        </Button>
      </div>
      <GitRow project={project} />
      {project.note && (
        <div className="flex items-center gap-2 text-[12px]">
          <span className="w-20 text-text-3">Note</span>
          <span className="truncate font-mono text-[11px] text-text-2">{project.note}</span>
        </div>
      )}
      <Panel title="Tasks" action={<span className="font-mono text-[11px] text-text-3">{mine.length}</span>}>
        <TaskBoard tasks={mine} compact label={`${project.name} tasks`} empty="No open tasks. Press c to add one." />
      </Panel>
      <DoneLog project={project} />
      <div className="mt-auto flex items-center gap-2 pt-4">
        {confirm ? (
          <>
            <span className="text-[12px] text-text-2">Delete {project.name}? Its tasks stay, without a project. The note stays in the vault.</span>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              Keep
            </Button>
            <Button onClick={() => void deleteProject(project.id)}>Delete</Button>
          </>
        ) : (
          <Button variant="ghost" onClick={() => setConfirm(true)}>
            Delete project
          </Button>
        )}
      </div>
    </div>
  );
}

export function Projects() {
  const { projects, tasks, selectedProject, openProject, setCreatingProject } = useStore();
  const current = projects.find((p) => p.id === selectedProject) ?? projects[0];
  const openCount = (id: number) => tasks.filter((t) => t.project_id === id && !isClosed(t.status)).length;
  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex w-[240px] shrink-0 flex-col border-r border-line">
        <header className="flex h-12 items-center px-4" data-tauri-drag-region>
          <h1 className="text-[13px] font-medium">Projects</h1>
          <Button variant="ghost" className="ml-auto" onClick={() => setCreatingProject(true)}>
            New project
          </Button>
        </header>
        <ul aria-label="Projects" className="flex flex-col gap-px overflow-y-auto px-2">
          {projects.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                onClick={() => openProject(p.id)}
                className={`flex h-8 w-full items-center gap-2 rounded-control px-2 text-left text-[12px] ${p.id === current?.id ? "bg-raised text-text" : "text-text-2 hover:bg-raised/60"}`}
              >
                <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${p.state === "active" ? "bg-text" : "border border-text-3"}`} aria-hidden="true" />
                <span className="truncate">{p.name}</span>
                <span className="ml-auto font-mono text-[11px] text-text-3">{p.prefix}</span>
                <span className="w-5 text-right font-mono text-[11px] text-text-3">{openCount(p.id)}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
      {current ? (
        <Detail key={current.id} project={current} />
      ) : (
        <div className="flex flex-1 flex-col items-start gap-3 p-8">
          <h2 className="text-[15px] font-medium">Start your first project</h2>
          <Notice>Pick a repo folder and Binder keeps its tasks and a note in your vault.</Notice>
          <Button variant="primary" onClick={() => setCreatingProject(true)}>
            New project
          </Button>
        </div>
      )}
    </div>
  );
}
