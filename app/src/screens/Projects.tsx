import { useEffect, useState } from "react";
import { COLOR_NAMES, PALETTE } from "../lib/colors";
import { ago } from "../lib/time";
import { Panel } from "../components/Panel";
import { ProjectDot } from "../components/ProjectDot";
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

function AgentDir({ project }: { project: Project }) {
  const { updateProject } = useStore();
  const [dir, setDir] = useState(project.agent_dir ?? "");
  useEffect(() => setDir(project.agent_dir ?? ""), [project.agent_dir]);
  const save = () => dir !== (project.agent_dir ?? "") && void updateProject(project.id, { agent_dir: dir });
  return (
    <label className="flex items-center gap-2 text-[12px]">
      <span className="w-20 shrink-0 text-text-3">Agent folder</span>
      <input
        value={dir}
        placeholder="~ on the server"
        onChange={(e) => setDir(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => e.key === "Enter" && save()}
        className="h-6 min-w-0 flex-1 bg-transparent font-mono text-[11px] text-text-2 placeholder:text-text-3 focus:text-text focus:outline-none"
      />
    </label>
  );
}

function Lock() {
  return (
    <svg role="img" aria-label="Private" viewBox="0 0 12 12" className="size-3 shrink-0 text-text-3" fill="none" stroke="currentColor" strokeWidth="1.2">
      <rect x="2.5" y="5.5" width="7" height="5" rx="1" />
      <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" />
    </svg>
  );
}

function ColorPicker({ project }: { project: Project }) {
  const { updateProject } = useStore();
  return (
    <div role="radiogroup" aria-label="Colour" className="flex items-center gap-1">
      {PALETTE.map((c) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-label={COLOR_NAMES[c]}
          aria-checked={project.color === c}
          onClick={() => void updateProject(project.id, { color: c })}
          className={`grid size-5 place-items-center rounded-full ${project.color === c ? "ring-1 ring-text-2" : "hover:ring-1 hover:ring-line-strong"}`}
        >
          <span className="size-2.5 rounded-full" style={{ background: c }} />
        </button>
      ))}
    </div>
  );
}

/** Where the project lives on GitHub: the link, privacy, language and last push. */
function GithubLine({ project }: { project: Project }) {
  const { api } = useStore();
  if (!project.github || !project.url) return null;
  const pushed = project.pushed_at ? Date.parse(project.pushed_at) / 1000 : NaN;
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <div className="flex min-w-0 items-center gap-3 text-[12px] text-text-2">
        <a
          href={project.url}
          onClick={(e) => {
            e.preventDefault();
            void api.openRepoPage(project.id);
          }}
          className="font-mono text-[11px] text-text hover:underline"
        >
          {project.github}
        </a>
        <span className="flex items-center gap-1">
          {project.private && <Lock />}
          {project.private ? "Private" : "Public"}
        </span>
        {project.language && <span>{project.language}</span>}
        {!Number.isNaN(pushed) && <span className="text-text-3">{`pushed ${ago(pushed)}`}</span>}
        {project.archived && <span className="text-text-3">Archived</span>}
      </div>
      {project.description && <p className="text-[12px] text-text-3">{project.description}</p>}
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
    <div className="flex min-w-0 max-w-[960px] flex-1 flex-col gap-4 overflow-y-auto p-5">
      <header className="flex items-center gap-3">
        <h1 className="text-[15px] font-medium">{project.name}</h1>
        <span className="font-mono text-[11px] text-text-3">{project.prefix}</span>
        <ColorPicker project={project} />
        <select
          aria-label="State"
          value={project.state}
          onChange={(e) => void updateProject(project.id, { state: e.target.value as ProjectState })}
          className="h-6 rounded-control border border-line bg-card px-1.5 text-[12px] text-text-2"
        >
          <option value="active">Active</option>
          <option value="paused">Paused</option>
          <option value="done">Done</option>
        </select>
        <Button variant="primary" className="ml-auto" onClick={() => void startOrOpen()}>
          {project.note ? "Open note" : "Start project"}
        </Button>
      </header>
      <GithubLine project={project} />
      {(vault.connected === false || error) && <Notice>{error && !error.startsWith("Vault not connected") ? error : vaultNotice(vault.path)}</Notice>}
      <div className="flex items-center gap-2 text-[12px]">
        <span className="w-20 text-text-3">Local folder</span>
        <span className="truncate font-mono text-[11px] text-text-2">{project.repo ?? "None"}</span>
        <Button variant="ghost" onClick={() => void pickFolder().then((f) => (f ? updateProject(project.id, { repo: f }) : null))}>
          Choose
        </Button>
      </div>
      <GitRow project={project} />
      <AgentDir project={project} />
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

function Row({ project, current, open }: { project: Project; current: boolean; open: number }) {
  const { openProject } = useStore();
  return (
    <li>
      <button
        type="button"
        onClick={() => openProject(project.id)}
        className={`flex h-8 w-full items-center gap-2 rounded-control px-2 text-left text-[12px] ${current ? "bg-raised text-text" : "text-text-2 hover:bg-raised/60"}`}
      >
        <ProjectDot color={project.color} />
        <span className="truncate">{project.name}</span>
        {project.private && <Lock />}
        <span className="ml-auto font-mono text-[11px] text-text-3">{project.prefix}</span>
        <span className="w-5 text-right font-mono text-[11px] text-text-3">{open}</span>
      </button>
    </li>
  );
}

function SyncLine() {
  const { github } = useStore();
  if (github?.error) return <p className="px-4 pb-2 text-[11px] text-urgent">{github.error}</p>;
  return <p className="px-4 pb-2 text-[11px] text-text-3">{github ? `Synced ${ago(github.at)}` : "Not synced yet"}</p>;
}

export function Projects() {
  const { projects, tasks, selectedProject, setCreatingProject, syncGithub, syncing } = useStore();
  const [showArchived, setShowArchived] = useState(false);
  const live = projects.filter((p) => !p.archived);
  const archived = projects.filter((p) => p.archived);
  const current = projects.find((p) => p.id === selectedProject) ?? live[0] ?? projects[0];
  const openCount = (id: number) => tasks.filter((t) => t.project_id === id && !isClosed(t.status)).length;
  return (
    <div className="flex min-h-0 flex-1">
      <div className="flex w-[240px] shrink-0 flex-col border-r border-line">
        <header className="flex h-12 items-center gap-1 px-4" data-tauri-drag-region>
          <h1 className="text-[13px] font-medium">Projects</h1>
          <Button variant="ghost" className="ml-auto" disabled={syncing} onClick={() => void syncGithub()}>
            Sync
          </Button>
          <Button variant="ghost" onClick={() => setCreatingProject(true)}>
            New project
          </Button>
        </header>
        <SyncLine />
        <ul aria-label="Projects" className="flex flex-col gap-px overflow-y-auto px-2 pb-2">
          {live.map((p) => (
            <Row key={p.id} project={p} current={p.id === current?.id} open={openCount(p.id)} />
          ))}
          {archived.length > 0 && (
            <li className="mt-2">
              <button
                type="button"
                aria-label={`Archived ${archived.length}`}
                aria-expanded={showArchived}
                onClick={() => setShowArchived((v) => !v)}
                className="flex h-7 w-full items-center gap-2 rounded-control px-2 text-left text-[11px] text-text-3 hover:text-text-2"
              >
                <span>Archived</span>
                <span className="ml-auto font-mono">{archived.length}</span>
              </button>
            </li>
          )}
          {showArchived && archived.map((p) => <Row key={p.id} project={p} current={p.id === current?.id} open={openCount(p.id)} />)}
        </ul>
      </div>
      {current ? (
        <Detail key={current.id} project={current} />
      ) : (
        <div className="flex flex-1 flex-col items-start gap-3 p-8">
          <h2 className="text-[15px] font-medium">Start your first project</h2>
          <Notice>Bring in your GitHub repos, or add a project by hand.</Notice>
          <div className="flex gap-2">
            <Button variant="primary" disabled={syncing} onClick={() => void syncGithub()}>
              Sync GitHub
            </Button>
            <Button variant="ghost" onClick={() => setCreatingProject(true)}>
              New project
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
