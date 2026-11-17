import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { runState } from "../components/AgentRun";
import { Terminal } from "../components/Terminal";
import { Button, fieldClass } from "../components/ui";
import { STATE_WORD, paneSession, projectPane, slug, tokens } from "../lib/agents";
import { loadPercent } from "../lib/load";
import { useStore } from "../lib/store";
import type { AgentSession, ClaudeUsage, Pane, PaneState, Project, RecentRun, ServerStatus } from "../lib/types";
import { whileVisible } from "../lib/usePoll";
import { gb } from "../lib/useServer";

/** One row of the sidebar: a pane and what people call it. */
interface Entry {
  pane: Pane;
  name: string;
  session: string;
  /** A project's session, which Close can end. */
  closable: boolean;
}

const DOT: Record<PaneState | "off", string> = {
  working: "bg-ok",
  waiting: "bg-st-progress",
  idle: "border border-text-3",
  exited: "border border-bad",
  off: "border border-line-strong",
};

function StateDot({ state }: { state: PaneState | "off" }) {
  return <span aria-hidden className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${DOT[state]}`} />;
}

/** Sessions on the left, one terminal on the right, like a small herdr. Stays mounted once opened. */
export function Agents({ shown }: { shown: boolean }) {
  const { api, setScreen } = useStore();
  const [target, setTarget] = useState<string | null | undefined>(undefined);
  const [model, setModel] = useState("qwen3:4b");
  const [projects, setProjects] = useState<Project[]>([]);
  const [live, setLive] = useState<AgentSession[]>([]);
  const [added, setAdded] = useState<string[]>([]);
  const [selected, setSelected] = useState("binder-claude");
  // Terminals connect when first shown and stay connected while hidden.
  const [opened, setOpened] = useState<string[]>(["binder-claude"]);
  const [error, setError] = useState<string | null>(null);
  const focus = useRef(new Map<string, () => void>());
  // One stable callback per session, so a terminal never registers twice.
  const registrars = useRef(new Map<string, (f: () => void) => void>());
  const registrar = (session: string) => {
    let r = registrars.current.get(session);
    if (!r) {
      r = (f) => focus.current.set(session, f);
      registrars.current.set(session, r);
    }
    return r;
  };

  // Read again each time the screen shows, so a server set in Settings is picked up.
  useEffect(() => {
    if (!shown) return;
    void api.getConfig().then((c) => {
      setTarget(c.server?.trim() ? c.server : null);
      if (c.ollama_model) setModel(c.ollama_model);
    });
    void api.listProjects().then(setProjects, () => {});
  }, [api, shown]);

  useEffect(() => {
    if (!shown || !target) return;
    const read = () => void api.agentSessions().then(setLive, () => {});
    read();
    const off = api.onAgentSessions(setLive);
    return () => off();
  }, [api, shown, target]);

  useEffect(() => {
    void api.agentViewing(shown ? selected : null).catch(() => {});
  }, [api, shown, selected]);

  const entries = useMemo<Entry[]>(() => {
    const list: Entry[] = [
      { pane: { kind: "claude", project: null, dir: null }, name: "Claude Code", session: "binder-claude", closable: false },
      { pane: { kind: "ollama", model }, name: "Ollama", session: "binder-ollama", closable: false },
    ];
    for (const p of projects) {
      if (!p.agent_dir) continue;
      const session = paneSession(projectPane(p));
      if (live.some((s) => s.session === session) || added.includes(session)) list.push({ pane: projectPane(p), name: p.name, session, closable: true });
    }
    return list;
  }, [projects, live, added, model]);

  const choose = useCallback((session: string) => {
    setSelected(session);
    setOpened((o) => (o.includes(session) ? o : [...o, session]));
    // The terminal may mount on this render; focus it on the next.
    setTimeout(() => focus.current.get(session)?.(), 0);
  }, []);

  useEffect(() => {
    if (!shown) return;
    const onKey = (e: KeyboardEvent) => {
      const n = Number(e.key);
      if (!e.metaKey || !Number.isInteger(n) || n < 1 || n > 9) return;
      const entry = entries[n - 1];
      if (!entry) return;
      e.preventDefault();
      choose(entry.session);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown, entries, choose]);

  const startProject = (id: string) => {
    const p = projects.find((x) => String(x.id) === id);
    if (!p) return;
    const session = paneSession(projectPane(p));
    setAdded((a) => (a.includes(session) ? a : [...a, session]));
    choose(session);
  };

  const close = async (e: Entry) => {
    setError(null);
    try {
      await api.closeAgentSession(e.session);
      setAdded((a) => a.filter((s) => s !== e.session));
      setOpened((o) => o.filter((s) => s !== e.session));
      setLive((l) => l.filter((s) => s.session !== e.session));
      if (selected === e.session) setSelected("binder-claude");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const pickModel = async (m: string) => {
    setError(null);
    try {
      await api.setOllamaModel(m);
      setOpened((o) => o.filter((s) => s !== "binder-ollama"));
      setModel(m);
      choose("binder-ollama");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const startable = projects.filter((p) => p.agent_dir && !p.archived && !entries.some((e) => e.session === `binder-claude-${slug(p.name)}`));

  return (
    <div hidden={!shown} className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Agents</h1>
        {target && <span className="font-mono text-[11px] text-text-3">{target}</span>}
        <span className="flex-1" data-tauri-drag-region />
        {target && shown && <Load />}
        {target && shown && <Usage />}
      </header>
      {target === null && (
        <div className="flex items-center gap-2 px-5 text-[12px] text-text-2">
          Set the home server in Settings to use Agents.
          <Button variant="ghost" onClick={() => setScreen("settings")}>
            Settings
          </Button>
        </div>
      )}
      {target && (
        <div className="flex min-h-0 flex-1 gap-3 px-5 pb-5">
          <nav aria-label="Sessions" className="flex w-52 shrink-0 flex-col gap-0.5">
            {entries.map((e, i) => {
              const s = live.find((x) => x.session === e.session);
              const state = s?.state ?? "off";
              const on = selected === e.session;
              return (
                <button
                  key={e.session}
                  type="button"
                  aria-current={on ? "true" : undefined}
                  onClick={() => choose(e.session)}
                  title={s?.line || undefined}
                  className={`flex items-center gap-2 rounded-control px-2 py-1.5 text-left text-[12px] ${on ? "bg-raised text-text" : "text-text-2 hover:bg-card"}`}
                >
                  <StateDot state={state} />
                  <span className="min-w-0 flex-1 truncate">{e.name}</span>
                  <span className={`text-[11px] ${state === "waiting" ? "text-st-progress" : "text-text-3"}`}>{STATE_WORD[state]}</span>
                  {i < 9 && <span className="font-mono text-[10px] text-text-3">⌘{i + 1}</span>}
                </button>
              );
            })}
            {startable.length > 0 && (
              <select aria-label="New session" className={`${fieldClass} mt-2`} value="" onChange={(e) => startProject(e.target.value)}>
                <option value="">New session…</option>
                {startable.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            )}
            {error && <p className="mt-2 text-[11px] text-bad">{error}</p>}
          </nav>
          <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
            {shown && <Runs />}
            {entries
              .filter((e) => opened.includes(e.session))
              .map((e) => (
                <div key={e.session === "binder-ollama" ? `ollama-${model}` : e.session} hidden={selected !== e.session} className="flex min-h-0 flex-1 flex-col">
                  <Terminal
                    pane={e.pane}
                    name={e.pane.kind === "ollama" ? `Ollama · ${model}` : e.closable ? `Claude Code · ${e.name}` : e.name}
                    registerFocus={registrar(e.session)}
                    actions={
                      e.pane.kind === "ollama" ? (
                        <Models model={model} onPick={pickModel} />
                      ) : e.closable ? (
                        <Button variant="ghost" onClick={() => void close(e)}>
                          Close
                        </Button>
                      ) : null
                    }
                  />
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** The server's CPU and memory, every 15 s while the screen shows. */
function Load() {
  const { api } = useStore();
  const [s, setS] = useState<ServerStatus | null>(null);
  useEffect(() => {
    const read = () => api.serverStatus().then(setS, () => {});
    void read();
    return whileVisible(read, 15_000);
  }, [api]);
  if (!s) return null;
  const cpu = loadPercent(s.load[0], s.cores) ?? s.load[0].toFixed(2);
  return (
    <span className="font-mono text-[11px] text-text-2" aria-label="Server load">
      CPU {cpu} · RAM {gb(s.mem_total - s.mem_available)}/{gb(s.mem_total)} GB
    </span>
  );
}

/** Claude Code's tokens on the server and this Mac, once a minute while the screen shows. */
function Usage() {
  const { api } = useStore();
  const [u, setU] = useState<ClaudeUsage | null>(null);
  useEffect(() => {
    const read = () => api.claudeUsage().then(setU, () => {});
    void read();
    return whileVisible(read, 60_000);
  }, [api]);
  if (!u) return null;
  return (
    <span className="font-mono text-[11px] text-text-2" aria-label="Claude Code usage" title="Input, output and cache writes, on the server and this Mac">
      Claude 5 h {tokens(u.five_hours)} · today {tokens(u.today)} · 7 days {tokens(u.week)}
    </span>
  );
}

/** The installed models, and any other name to pull. */
function Models({ model, onPick }: { model: string; onPick(m: string): void }) {
  const { api } = useStore();
  const [models, setModels] = useState<string[]>([]);
  const [other, setOther] = useState(false);
  const [name, setName] = useState("");
  useEffect(() => {
    void api.ollamaModels().then(setModels, () => {});
  }, [api, model]);
  if (other)
    return (
      <form
        className="flex items-center gap-1"
        onSubmit={(e) => {
          e.preventDefault();
          if (name.trim()) onPick(name.trim());
          setOther(false);
        }}
      >
        <input aria-label="Model name" autoFocus className={`${fieldClass} w-40`} placeholder="llama3.2" value={name} onChange={(e) => setName(e.target.value)} />
        <Button variant="ghost" type="submit">
          Run
        </Button>
      </form>
    );
  const list = models.includes(model) ? models : [model, ...models];
  return (
    <select
      aria-label="Model"
      className={`${fieldClass} w-40`}
      value={model}
      onChange={(e) => (e.target.value === "" ? setOther(true) : onPick(e.target.value))}
    >
      {list.map((m) => (
        <option key={m} value={m}>
          {m}
        </option>
      ))}
      <option value="">Other…</option>
    </select>
  );
}

const minutes = (from: number, to: number) => {
  const m = Math.max(0, Math.round((to - from) / 60));
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
};

/** Agent runs that are live or ended in the last day. Live ones are read every 15 s. */
function Runs() {
  const { api } = useStore();
  const [runs, setRuns] = useState<RecentRun[]>([]);
  const read = useCallback(async () => {
    const list = await api.recentRuns().catch(() => null);
    if (!list) return;
    for (const r of list.filter((x) => x.run.ended === null && !x.run.queued)) await api.pollRun(r.run.run).catch(() => {});
    setRuns((await api.recentRuns().catch(() => list)) ?? list);
  }, [api]);
  useEffect(() => {
    void read();
    return whileVisible(read, 15_000);
  }, [read]);
  if (runs.length === 0) return null;
  const now = Math.floor(Date.now() / 1000);
  return (
    <section aria-label="Agent runs" className="flex max-h-40 shrink-0 flex-col overflow-y-auto rounded-panel border border-line">
      {runs.map((r) => {
        const last = r.run.tail.trim().split("\n").pop() ?? "";
        const live = r.run.ended === null;
        return (
          <div key={r.run.run} className="flex items-center gap-3 border-b border-line px-3 py-1.5 text-[12px] last:border-b-0">
            <span className="font-mono text-[11px] text-text-3">{r.key}</span>
            <span className="min-w-0 max-w-64 truncate">{r.title}</span>
            <span className="shrink-0 text-[11px] text-text-2">{runState(r.run)}</span>
            <span className="shrink-0 font-mono text-[11px] text-text-3">{minutes(r.run.started, r.run.ended ?? now)}</span>
            <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-text-3">{last}</span>
            {live && (
              <Button variant="ghost" onClick={() => void api.stopRun(r.run.run).then(read, () => {})}>
                Stop
              </Button>
            )}
          </div>
        );
      })}
    </section>
  );
}
