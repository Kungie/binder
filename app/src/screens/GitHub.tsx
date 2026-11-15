import { useCallback, useEffect, useState } from "react";
import { CiIcon } from "../components/CiIcon";
import { ProjectDot } from "../components/ProjectDot";
import { Button, Notice, fieldClass } from "../components/ui";
import { useStore } from "../lib/store";
import { ago } from "../lib/time";
import type { AgentReady, GhAction, GhDetail, GhFilter, GhLists, IssueRow, Project, PullRow, Review } from "../lib/types";

const EVERY = 120_000;
/** The field look, sized to its content. */
const pickerClass = fieldClass.replace("w-full", "w-auto");
const FILTERS: [GhFilter, string][] = [
  ["needs_you", "Needs you"],
  ["mine", "Mine"],
  ["all_open", "All open"],
];
const REVIEW: Record<Review, string> = { approved: "Approved", changes_requested: "Changes requested", review_required: "Review requested", draft: "Draft" };
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const updated = (iso: string) => ago(Date.parse(iso) / 1000);

type Picked = { kind: "pull"; row: PullRow } | { kind: "issue"; row: IssueRow };

/** Pull requests and issues across the owner's projects, answered from Binder, with the agent's help when asked. */
export function GitHub() {
  const { api, projects } = useStore();
  const [tab, setTab] = useState<"pulls" | "issues">("pulls");
  const [filter, setFilter] = useState<GhFilter>("needs_you");
  const [projectId, setProjectId] = useState<number | null>(null);
  const [lists, setLists] = useState<GhLists | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [picked, setPicked] = useState<Picked | null>(null);
  const [creating, setCreating] = useState(false);
  const onGithub = projects.filter((p) => p.github && !p.archived);
  const projectOf = (repo: string) => projects.find((p) => p.github?.toLowerCase() === repo.toLowerCase());

  const load = useCallback(async () => {
    try {
      setLists(await api.ghList(filter, projectId));
      setError(null);
    } catch (e) {
      setError(message(e));
    }
  }, [api, filter, projectId]);

  useEffect(() => {
    void load();
    const t = window.setInterval(() => void load(), EVERY);
    return () => window.clearInterval(t);
  }, [load]);

  const rows = tab === "pulls" ? (lists?.pulls ?? []) : (lists?.issues ?? []);
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">GitHub</h1>
        <div role="tablist" aria-label="Kind" className="flex items-center gap-1">
          {(["pulls", "issues"] as const).map((k) => (
            <button
              key={k}
              type="button"
              role="tab"
              aria-selected={tab === k}
              onClick={() => {
                setTab(k);
                setPicked(null);
              }}
              className={`flex h-7 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-control px-2.5 text-[12px] ${tab === k ? "bg-raised text-text" : "text-text-2 hover:text-text"}`}
            >
              {k === "pulls" ? "Pull requests" : "Issues"}
              <span className="font-mono text-[11px] text-text-3">{k === "pulls" ? (lists?.pulls.length ?? 0) : (lists?.issues.length ?? 0)}</span>
            </button>
          ))}
        </div>
        <div role="radiogroup" aria-label="Filter" className="flex shrink-0 items-center rounded-control border border-line p-0.5">
          {FILTERS.map(([f, label]) => (
            <button
              key={f}
              type="button"
              role="radio"
              aria-checked={filter === f}
              onClick={() => setFilter(f)}
              className={`h-6 whitespace-nowrap rounded-[6px] px-2 text-[12px] ${filter === f ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
            >
              {label}
            </button>
          ))}
        </div>
        <select aria-label="Project" className={`${pickerClass} max-w-40`} value={projectId ?? ""} onChange={(e) => setProjectId(e.target.value ? Number(e.target.value) : null)}>
          <option value="">All projects</option>
          {onGithub.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <span className="ml-auto flex shrink-0 items-center gap-2 whitespace-nowrap">
          <Button variant="ghost" onClick={() => void load()}>
            Refresh
          </Button>
          <Button onClick={() => setCreating(true)}>New issue</Button>
        </span>
      </header>
      {error && (
        <div className="px-5 pb-3">
          <Notice>{error}</Notice>
        </div>
      )}
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,380px)_minmax(0,1fr)] gap-3 px-5 pb-5">
        <div className="min-h-0 overflow-y-auto rounded-panel border border-line bg-panel">
          <ul aria-label={tab === "pulls" ? "Pull requests" : "Issues"}>
            {rows.map((r) => {
              const pull = tab === "pulls";
              const p = projectOf(r.repo);
              const on = picked?.row.repo === r.repo && picked.row.number === r.number;
              return (
                <li key={`${r.repo}#${r.number}`} className="border-b border-line last:border-b-0">
                  <button
                    type="button"
                    aria-pressed={on}
                    onClick={() => setPicked(pull ? { kind: "pull", row: r as PullRow } : { kind: "issue", row: r as IssueRow })}
                    className={`flex w-full flex-col gap-1 px-3 py-2 text-left text-[12px] ${on ? "bg-raised" : "hover:bg-raised/60"}`}
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      {p && <ProjectDot color={p.color} />}
                      <span className="font-mono text-[11px] text-text-3">{`#${r.number}`}</span>
                      <span className="min-w-0 flex-1 truncate text-text">{r.title}</span>
                      {pull && (r as PullRow).ci && <CiIcon ci={(r as PullRow).ci!} />}
                    </span>
                    <span className="flex min-w-0 items-center gap-2 pl-[22px] text-[11px] text-text-3">
                      <span className="truncate">{r.author}</span>
                      {pull && (r as PullRow).review && <span className="text-text-2">{REVIEW[(r as PullRow).review!]}</span>}
                      {!pull && (r as IssueRow).assignees.length > 0 && <span>{`→ ${(r as IssueRow).assignees.join(", ")}`}</span>}
                      <span className="ml-auto shrink-0">{updated(r.updated)}</span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
          {lists && rows.length === 0 && <p className="px-3 py-3 text-[12px] text-text-3">{filter === "needs_you" ? "Nothing needs you." : "Nothing open."}</p>}
        </div>
        <div className="min-h-0 overflow-y-auto">
          {picked ? (
            <DetailView key={`${picked.row.repo}#${picked.row.number}`} picked={picked} project={projectOf(picked.row.repo)} onChanged={load} />
          ) : (
            <p className="px-1 py-3 text-[12px] text-text-3">Pick a pull request or an issue.</p>
          )}
        </div>
      </div>
      {creating && <NewIssue projects={onGithub} initial={projectId} onClose={() => setCreating(false)} onCreated={load} />}
    </div>
  );
}

/** The agent's review runs by pull request, for each Api, so leaving a pull request while it runs keeps its draft coming. */
const reviewRuns = new WeakMap<object, Map<string, string>>();
function runsFor(api: object) {
  let m = reviewRuns.get(api);
  if (!m) reviewRuns.set(api, (m = new Map()));
  return m;
}

function DetailView({ picked, project, onChanged }: { picked: Picked; project: Project | undefined; onChanged(): void }) {
  const { api } = useStore();
  const pull = picked.kind === "pull";
  const { repo, number, title } = picked.row;
  const [detail, setDetail] = useState<GhDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [confirm, setConfirm] = useState<"merge" | "close" | null>(null);
  const [working, setWorking] = useState(false);
  const [closed, setClosed] = useState(false);
  const [ready, setReady] = useState<AgentReady | null>(null);
  const reviewKey = `${repo}#${number}`;
  const [reviewRun, setRun] = useState<string | null>(() => runsFor(api).get(reviewKey) ?? null);
  const setReviewRun = (run: string | null) => {
    if (run) runsFor(api).set(reviewKey, run);
    else runsFor(api).delete(reviewKey);
    setRun(run);
  };
  const [draft, setDraft] = useState<string | null>(null);
  /** Posting or discarding the draft ends the review. */
  const endReview = () => {
    setReviewRun(null);
    setDraft(null);
  };
  const [started, setStarted] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      setDetail(await api.ghDetail(repo, number, pull));
    } catch (e) {
      setError(message(e));
    }
  }, [api, repo, number, pull]);
  useEffect(() => {
    void read();
  }, [read]);
  useEffect(() => {
    if (project) void api.ghAgentReady(project.id).then(setReady, () => setReady({ gh: false, folder: false }));
  }, [api, project]);
  // The agent's review arrives when its run ends; until then it is asked for every 10 seconds.
  useEffect(() => {
    if (!reviewRun || draft !== null) return;
    let live = true;
    const ask = () =>
      void api.ghReviewDraft(reviewRun).then(
        (d) => live && d !== null && setDraft(d),
        () => {},
      );
    ask();
    const t = window.setInterval(ask, 10_000);
    return () => {
      live = false;
      window.clearInterval(t);
    };
  }, [api, reviewRun, draft]);

  const act = async (action: GhAction, after?: () => void) => {
    setWorking(true);
    setConfirm(null);
    try {
      await api.ghAct(repo, number, pull, action);
      after?.();
      setError(null);
      await read();
      onChanged();
    } catch (e) {
      setError(message(e));
    } finally {
      setWorking(false);
    }
  };
  const agent = async (start: () => Promise<{ run: string }>, review: boolean) => {
    try {
      const run = await start();
      if (review) setReviewRun(run.run);
      else setStarted("The agent is on it. Its run shows on the task.");
      setError(null);
    } catch (e) {
      setError(message(e));
    }
  };
  const why = !ready ? null : !ready.folder ? "Set the project's agent folder first." : null;
  const pushWhy = !ready ? null : (why ?? (!ready.gh ? "Sign in to gh on the server to let the agent push." : null));
  const row = picked.row as PullRow;
  const needsFix = pull && (row.ci === "failing" || row.review === "changes_requested");
  const base = detail?.base ?? "main";

  return (
    <section aria-label={`#${number} ${title}`} className="flex flex-col gap-3 rounded-panel border border-line bg-panel p-4">
      <div className="flex items-start gap-2">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="font-mono text-[11px] text-text-3">{`${repo} #${number}`}</span>
          <h2 className="text-[15px] font-medium text-text">{title}</h2>
        </div>
        {detail && (
          <Button variant="ghost" onClick={() => void api.openLink(detail.url)}>
            Open on GitHub
          </Button>
        )}
      </div>
      {error && <Notice>{error}</Notice>}
      {detail && pull && (
        <div className="flex flex-wrap items-center gap-3 text-[12px] text-text-2">
          {detail.head && <span className="font-mono text-[11px]">{`${detail.head} → ${base}`}</span>}
          <span className="font-mono text-[11px]">{`+${detail.additions} −${detail.deletions} in ${detail.files} ${detail.files === 1 ? "file" : "files"}`}</span>
        </div>
      )}
      {detail && detail.checks.length > 0 && (
        <ul aria-label="Checks" className="flex flex-wrap gap-2">
          {detail.checks.map(([name, ci], i) => (
            <li key={`${name}-${i}`} className="flex h-6 items-center gap-1.5 rounded-full border border-line px-2 text-[11px] text-text-2">
              {ci && <CiIcon ci={ci} />}
              {name}
            </li>
          ))}
        </ul>
      )}
      {detail?.body && <p className="whitespace-pre-wrap text-[13px] text-text-2">{detail.body}</p>}
      {detail && detail.comments.length > 0 && (
        <ul aria-label="Comments" className="flex flex-col gap-2 border-t border-line pt-3">
          {detail.comments.map((c, i) => (
            <li key={i} className="flex flex-col gap-0.5 text-[12px]">
              <span className="flex items-center gap-2 text-text-3">
                <span className="text-text-2">{c.author}</span>
                {c.state && c.state !== "COMMENTED" && <span>{c.state === "APPROVED" ? "approved" : c.state === "CHANGES_REQUESTED" ? "asked for changes" : c.state.toLowerCase()}</span>}
                <span>{updated(c.at)}</span>
              </span>
              {c.body && <span className="whitespace-pre-wrap text-text">{c.body}</span>}
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-col gap-2 border-t border-line pt-3">
        {reviewRun && draft === null && <span className="text-[12px] text-text-3">The agent is reviewing. Its draft shows here when it's done.</span>}
        {draft !== null && (
          <div className="flex flex-col gap-2">
            <textarea aria-label="Agent's review" value={draft} onChange={(e) => setDraft(e.target.value)} rows={6} className={`${fieldClass} h-auto py-1.5 font-mono text-[11px]`} />
            <div className="flex flex-wrap items-center gap-2">
              <Button variant="primary" disabled={working} onClick={() => void act({ kind: "comment", body: draft }, () => endReview())}>
                Post as comment
              </Button>
              <Button disabled={working} onClick={() => void act({ kind: "approve", body: draft }, () => endReview())}>
                Approve
              </Button>
              <Button disabled={working} onClick={() => void act({ kind: "request_changes", body: draft }, () => endReview())}>
                Request changes
              </Button>
              <Button variant="ghost" onClick={() => endReview()}>
                Discard
              </Button>
            </div>
          </div>
        )}
        <textarea aria-label="Comment" placeholder="Write a comment" value={text} onChange={(e) => setText(e.target.value)} rows={3} className={`${fieldClass} h-auto py-1.5`} />
        {confirm ? (
          <div className="flex items-center gap-2 text-[12px]">
            <span className="text-text-2">{confirm === "merge" ? `Merge #${number} into ${base}?` : `Close #${number}?`}</span>
            <Button variant="primary" onClick={() => void act({ kind: confirm }, () => confirm === "close" && setClosed(true))}>
              {confirm === "merge" ? "Merge" : "Close"}
            </Button>
            <Button variant="ghost" onClick={() => setConfirm(null)}>
              Cancel
            </Button>
          </div>
        ) : (
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="primary" disabled={working || !text.trim()} onClick={() => void act({ kind: "comment", body: text }, () => setText(""))}>
              Comment
            </Button>
            {pull && (
              <>
                <Button disabled={working} onClick={() => void act({ kind: "approve", body: text }, () => setText(""))}>
                  Approve
                </Button>
                <Button disabled={working || !text.trim()} onClick={() => void act({ kind: "request_changes", body: text }, () => setText(""))}>
                  Request changes
                </Button>
                <Button disabled={working || closed} onClick={() => setConfirm("merge")}>
                  Merge
                </Button>
              </>
            )}
            {!pull && !(picked.row as IssueRow).assignees.length && (
              <Button disabled={working} onClick={() => void act({ kind: "assign_me" })}>
                Assign to me
              </Button>
            )}
            {closed ? (
              <Button disabled={working} onClick={() => void act({ kind: "reopen" }, () => setClosed(false))}>
                Reopen
              </Button>
            ) : (
              <Button variant="ghost" disabled={working} onClick={() => setConfirm("close")}>
                Close
              </Button>
            )}
          </div>
        )}
      </div>

      {project && (
        <div className="flex flex-col gap-2 border-t border-line pt-3">
          <span className="text-[11px] text-text-3">The agent</span>
          <div className="flex flex-wrap items-center gap-2">
            {pull ? (
              <>
                <Button disabled={!ready || !!why || !!reviewRun} onClick={() => void agent(() => api.ghReview(project.id, repo, number), true)}>
                  Review with the agent
                </Button>
                {needsFix && (
                  <Button disabled={!ready || !!pushWhy} onClick={() => void agent(() => api.ghFix(project.id, repo, number), false)}>
                    Fix with the agent
                  </Button>
                )}
              </>
            ) : (
              <Button disabled={!ready || !!pushWhy} onClick={() => void agent(() => api.ghWork(project.id, repo, number, title), false)}>
                Work on it
              </Button>
            )}
          </div>
          {(why ?? ((!pull || needsFix) && pushWhy)) && <span className="text-[12px] text-text-3">{why ?? pushWhy}</span>}
          {started && <span className="text-[12px] text-text-2">{started}</span>}
        </div>
      )}
    </section>
  );
}

function NewIssue({ projects, initial, onClose, onCreated }: { projects: Project[]; initial: number | null; onClose(): void; onCreated(): void }) {
  const { api } = useStore();
  const [projectId, setProjectId] = useState<number | "">(initial ?? projects[0]?.id ?? "");
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  const create = async () => {
    if (projectId === "") return setError("Pick a project.");
    if (!title.trim()) return setError("Give it a title.");
    try {
      await api.ghNewIssue(projectId, title.trim(), body);
      onCreated();
      onClose();
    } catch (e) {
      setError(message(e));
    }
  };
  return (
    <div className="backdrop-in fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[14vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-label="New issue"
        className="pop-in w-[560px] max-w-[calc(100vw-32px)] rounded-panel border border-line-strong bg-panel p-4 shadow-menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") onClose();
          if (e.key === "Enter" && e.metaKey) void create();
        }}
      >
        <input autoFocus aria-label="Issue title" placeholder="Issue title" value={title} onChange={(e) => setTitle(e.target.value)} className="w-full bg-transparent text-[15px] text-text placeholder:text-text-3 focus:outline-none" />
        <textarea aria-label="Issue body" placeholder="Add a description" rows={4} value={body} onChange={(e) => setBody(e.target.value)} className="mt-2 w-full resize-none bg-transparent text-[13px] text-text-2 placeholder:text-text-3 focus:outline-none" />
        <div className="mt-4 flex items-center gap-2 border-t border-line pt-3">
          <select aria-label="Project" className={`${pickerClass} max-w-40`} value={projectId} onChange={(e) => setProjectId(e.target.value ? Number(e.target.value) : "")}>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          {error && <span className="min-w-0 truncate text-[12px] text-text-2">{error}</span>}
          <Button variant="ghost" className="ml-auto" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => void create()}>
            Create issue
          </Button>
        </div>
      </div>
    </div>
  );
}
