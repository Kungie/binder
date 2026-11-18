import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { runState } from "../components/AgentRun";
import { Terminal } from "../components/Terminal";
import { Button, fieldClass } from "../components/ui";
import { STATE_COLOR, STATE_WORD, paneSession, projectPane, slug, tokens } from "../lib/agents";
import { loadPercent } from "../lib/load";
import { useStore } from "../lib/store";
import type { AgentSession, ClaudeLimits, ClaudeUsage, Config, LimitWindow, Pane, PaneState, Project, RecentRun, ServerStatus } from "../lib/types";
import { whileVisible } from "../lib/usePoll";
import { gb } from "../lib/useServer";

/** One row of the sidebar: a pane and what people call it. */
interface Entry {
  pane: Pane;
  name: string;
  session: string;
  /** Claude Code and Ollama stay listed when ended; a project's session is listed while it runs or was opened;
   *  a session made here is listed until closed; any other `binder-*` session while it runs. */
  kind: "builtin" | "project" | "saved" | "other";
}

type Shown = PaneState | "off" | "error";

function StateDot({ state }: { state: Shown }) {
  return <span aria-hidden className={`inline-block h-2 w-2 shrink-0 rounded-full ${STATE_COLOR[state].dot}`} />;
}

const errorText = (err: unknown) => (err instanceof Error ? err.message : String(err));

/** Sessions on the left, one terminal on the right, like a small herdr. Stays mounted once opened. */
export function Agents({ shown }: { shown: boolean }) {
  const { api, setScreen } = useStore();
  const [target, setTarget] = useState<string | null | undefined>(undefined);
  const [model, setModel] = useState("qwen3:4b");
  const [saved, setSaved] = useState<{ name: string; dir: string }[]>([]);
  const [names, setNames] = useState<Record<string, string>>({});
  const [projects, setProjects] = useState<Project[]>([]);
  const [live, setLive] = useState<AgentSession[]>([]);
  const [unreachable, setUnreachable] = useState(false);
  const [added, setAdded] = useState<string[]>([]);
  const [selected, setSelected] = useState("binder-claude");
  // Terminals connect when first shown and stay connected while hidden.
  const [opened, setOpened] = useState<string[]>(["binder-claude"]);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [closing, setClosing] = useState<string | null>(null);
  const [making, setMaking] = useState(false);
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
      setSaved(c.agents ?? []);
      setNames(c.agent_names ?? {});
    });
    void api.listProjects().then(setProjects, () => {});
  }, [api, shown]);

  useEffect(() => {
    if (!shown || !target) return;
    const got = (l: AgentSession[]) => {
      setLive(l);
      setUnreachable(false);
    };
    void api.agentSessions().then(got, () => setUnreachable(true));
    const off = api.onAgentSessions(got);
    return () => off();
  }, [api, shown, target]);

  useEffect(() => {
    void api.agentViewing(shown ? selected : null).catch(() => {});
  }, [api, shown, selected]);

  const entries = useMemo<Entry[]>(() => {
    const list: Entry[] = [
      { pane: { kind: "claude", project: null, dir: null }, name: "Claude Code", session: "binder-claude", kind: "builtin" },
      { pane: { kind: "ollama", model }, name: "Ollama", session: "binder-ollama", kind: "builtin" },
    ];
    const has = (session: string) => list.some((e) => e.session === session);
    for (const p of projects) {
      if (!p.agent_dir) continue;
      const session = paneSession(projectPane(p));
      if (!has(session) && (live.some((s) => s.session === session) || added.includes(session))) list.push({ pane: projectPane(p), name: p.name, session, kind: "project" });
    }
    for (const a of saved) {
      const pane: Pane = { kind: "claude", project: a.name, dir: a.dir };
      const session = paneSession(pane);
      if (!has(session)) list.push({ pane, name: a.name, session, kind: "saved" });
    }
    for (const s of live) {
      if (!has(s.session)) list.push({ pane: { kind: "attach", session: s.session }, name: s.session.replace(/^binder-/, ""), session: s.session, kind: "other" });
    }
    return list.map((e) => ({ ...e, name: names[e.session] ?? e.name }));
  }, [projects, live, added, model, saved, names]);

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

  /** Reads the config again and changes only the Agents fields, so other screens' settings stay. */
  const saveConfig = async (change: (c: Config) => Config) => {
    const c = change(await api.getConfig());
    await api.setConfig(c);
    setSaved(c.agents ?? []);
    setNames(c.agent_names ?? {});
  };

  const startProject = (id: string) => {
    const p = projects.find((x) => String(x.id) === id);
    if (!p) return;
    const session = paneSession(projectPane(p));
    setAdded((a) => (a.includes(session) ? a : [...a, session]));
    choose(session);
  };

  const make = async (name: string, dir: string, asProject: boolean) => {
    setError(null);
    const folder = dir.trim() || `~/code/${slug(name)}`;
    const session = paneSession({ kind: "claude", project: name, dir: folder });
    if (entries.some((e) => e.session === session)) {
      choose(session);
      setMaking(false);
      return;
    }
    try {
      if (asProject) {
        const p = await api.createProject({ name });
        await api.updateProject(p.id, { agent_dir: folder });
        setProjects(await api.listProjects());
        setAdded((a) => [...a, session]);
      } else {
        await saveConfig((c) => ({ ...c, agents: [...(c.agents ?? []), { name, dir: folder }] }));
      }
      setMaking(false);
      choose(session);
    } catch (err) {
      setError(errorText(err));
    }
  };

  const rename = async (session: string, name: string) => {
    setRenaming(null);
    try {
      await saveConfig((c) => {
        const all = { ...(c.agent_names ?? {}) };
        if (name.trim()) all[session] = name.trim();
        else delete all[session];
        return { ...c, agent_names: all };
      });
    } catch (err) {
      setError(errorText(err));
    }
  };

  const close = async (e: Entry) => {
    setError(null);
    setClosing(null);
    try {
      if (live.some((s) => s.session === e.session) || opened.includes(e.session)) await api.closeAgentSession(e.session);
      if (e.kind === "saved")
        await saveConfig((c) => {
          const all = { ...(c.agent_names ?? {}) };
          delete all[e.session];
          return { ...c, agents: (c.agents ?? []).filter((a) => paneSession({ kind: "claude", project: a.name, dir: a.dir }) !== e.session), agent_names: all };
        });
      setAdded((a) => a.filter((s) => s !== e.session));
      setOpened((o) => o.filter((s) => s !== e.session));
      setLive((l) => l.filter((s) => s.session !== e.session));
      if (selected === e.session && e.kind !== "builtin") setSelected("binder-claude");
    } catch (err) {
      setError(errorText(err));
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
      setError(errorText(err));
    }
  };

  const startable = projects.filter((p) => p.agent_dir && !p.archived && !entries.some((e) => e.session === paneSession(projectPane(p))));
  const current = entries.find((e) => e.session === selected);

  return (
    <div hidden={!shown} className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-4 px-5" data-tauri-drag-region>
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
          <nav aria-label="Sessions" className="flex w-56 shrink-0 flex-col gap-0.5 overflow-y-auto">
            {entries.map((e, i) => {
              const s = live.find((x) => x.session === e.session);
              const state: Shown = unreachable ? "error" : (s?.state ?? "off");
              const on = selected === e.session;
              if (renaming === e.session) return <RenameField key={e.session} name={e.name} onDone={(n) => (n === null ? setRenaming(null) : void rename(e.session, n))} />;
              return (
                <button
                  key={e.session}
                  type="button"
                  aria-current={on ? "true" : undefined}
                  onClick={() => choose(e.session)}
                  onDoubleClick={() => setRenaming(e.session)}
                  title={unreachable ? "Couldn't read the server." : s?.line || undefined}
                  className={`flex items-center gap-2 rounded-control px-2 py-1.5 text-left text-[12px] ${on ? "bg-raised text-text" : "text-text-2 hover:bg-card"}`}
                >
                  <StateDot state={state} />
                  <span className="min-w-0 flex-1 truncate">{e.name}</span>
                  <span className={`text-[11px] ${STATE_COLOR[state].text}`}>{STATE_WORD[state]}</span>
                  {i < 9 && <span className="font-mono text-[10px] text-text-3">⌘{i + 1}</span>}
                </button>
              );
            })}
            {making ? (
              <NewSession onMake={(n, d, p) => void make(n, d, p)} onCancel={() => setMaking(false)} />
            ) : (
              <Button variant="ghost" className="mt-2 justify-start" onClick={() => setMaking(true)}>
                New session
              </Button>
            )}
            {startable.length > 0 && (
              <select aria-label="Open project" className={`${fieldClass} mt-1`} value="" onChange={(e) => startProject(e.target.value)}>
                <option value="">Open a project…</option>
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
            {current && !opened.includes(current.session) && (
              <div className="flex flex-1 items-center justify-center rounded-panel border border-line text-[12px] text-text-3">
                <Button variant="ghost" onClick={() => choose(current.session)}>
                  Open {current.name}
                </Button>
              </div>
            )}
            {entries
              .filter((e) => opened.includes(e.session))
              .map((e) => (
                <div key={e.session === "binder-ollama" ? `ollama-${model}` : e.session} hidden={selected !== e.session} className="flex min-h-0 flex-1 flex-col">
                  <Terminal
                    pane={e.pane}
                    name={e.pane.kind === "ollama" ? `${e.name} · ${model}` : e.kind === "project" || e.kind === "saved" ? `Claude Code · ${e.name}` : e.name}
                    registerFocus={registrar(e.session)}
                    actions={
                      <span className="flex items-center gap-1">
                        {e.pane.kind === "ollama" && <Models model={model} onPick={pickModel} />}
                        {closing === e.session ? (
                          <>
                            <span className="text-[12px] text-text-2">End this session?</span>
                            <Button variant="ghost" onClick={() => void close(e)}>
                              End
                            </Button>
                            <Button variant="ghost" onClick={() => setClosing(null)}>
                              Keep
                            </Button>
                          </>
                        ) : (
                          <>
                            <Button variant="ghost" onClick={() => setRenaming(e.session)}>
                              Rename
                            </Button>
                            <Button variant="ghost" onClick={() => setClosing(e.session)}>
                              Close
                            </Button>
                          </>
                        )}
                      </span>
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

/** The sidebar row while it is renamed: Enter keeps the name, Escape drops it, blank goes back to the default. */
function RenameField({ name, onDone }: { name: string; onDone(name: string | null): void }) {
  const [text, setText] = useState(name);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onDone(text);
      }}
    >
      <input
        aria-label="Session name"
        autoFocus
        className={`${fieldClass} w-full`}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && onDone(null)}
        onBlur={() => onDone(null)}
      />
    </form>
  );
}

/** A new Claude Code session: a name, the server folder it starts in, and whether it is a new project too. */
function NewSession({ onMake, onCancel }: { onMake(name: string, dir: string, asProject: boolean): void; onCancel(): void }) {
  const [name, setName] = useState("");
  const [dir, setDir] = useState("");
  const [asProject, setAsProject] = useState(false);
  return (
    <form
      aria-label="New session"
      className="mt-2 flex flex-col gap-1.5 rounded-panel border border-line p-2"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim()) onMake(name.trim(), dir, asProject);
      }}
    >
      <input aria-label="Name" autoFocus className={fieldClass} placeholder="Name" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Escape" && onCancel()} />
      <input
        aria-label="Folder"
        className={`${fieldClass} font-mono`}
        placeholder={`~/code/${name.trim() ? slug(name) : "name"}`}
        value={dir}
        onChange={(e) => setDir(e.target.value)}
      />
      <label className="flex items-center gap-2 text-[12px] text-text-2">
        <input type="checkbox" checked={asProject} onChange={(e) => setAsProject(e.target.checked)} />
        Add to Projects
      </label>
      <span className="flex gap-1">
        <Button type="submit" disabled={!name.trim()}>
          Start
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
      </span>
    </form>
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

/** Green under half, yellow under 80 %, red above. */
const level = (p: number) => (p >= 80 ? { bar: "bg-bad", text: "text-bad" } : p >= 50 ? { bar: "bg-st-progress", text: "text-st-progress" } : { bar: "bg-ok", text: "text-ok" });

function resets(at: number | null): string {
  if (at === null) return "";
  const d = new Date(at * 1000);
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const sameDay = d.toDateString() === new Date().toDateString();
  return `Resets ${sameDay ? "" : `${d.toLocaleDateString([], { weekday: "short" })} `}${time}`;
}

function Meter({ label, w }: { label: string; w: LimitWindow }) {
  const c = level(w.percent);
  return (
    <span className="flex items-center gap-1.5" title={resets(w.resets_at)}>
      <span className="text-text-3">{label}</span>
      <span aria-hidden className="h-1.5 w-12 overflow-hidden rounded-full bg-raised">
        <span className={`block h-full rounded-full ${c.bar}`} style={{ width: `${Math.min(100, Math.max(2, w.percent))}%` }} />
      </span>
      <span className={c.text}>{`${Math.round(w.percent)}%`}</span>
    </span>
  );
}

/** Claude Code's plan limits in percent when Binder's status line is on the server; the tokens otherwise. Read once a
 *  minute while the screen shows. */
function Usage() {
  const { api } = useStore();
  const [u, setU] = useState<ClaudeUsage | null>(null);
  const [l, setL] = useState<ClaudeLimits | null>(null);
  const tried = useRef(false);
  const read = useCallback(() => {
    void api.claudeUsage().then(setU, () => {});
    void api.claudeLimits().then(setL, () => {});
  }, [api]);
  useEffect(() => {
    read();
    return whileVisible(read, 60_000);
  }, [read]);
  // Binder's status line goes on the server by itself, once, the first time it's missing.
  useEffect(() => {
    if (!l || l.installed || tried.current) return;
    tried.current = true;
    void api.installClaudeLimits().then(read, () => {});
  }, [api, l, read]);
  const sums = u ? `5 h ${tokens(u.five_hours)} · today ${tokens(u.today)} · 7 days ${tokens(u.week)} tokens` : "";
  const lim = l?.limits;
  return (
    <span className="flex items-center gap-3 font-mono text-[11px]" aria-label="Claude Code usage" title={sums}>
      <span className="text-text-2">Claude</span>
      {lim && (lim.five_hour || lim.seven_day) ? (
        <>
          {lim.five_hour && <Meter label="5 h" w={lim.five_hour} />}
          {lim.seven_day && <Meter label="week" w={lim.seven_day} />}
        </>
      ) : (
        <>
          {u && (
            <span className="text-text-3">
              5 h <span className="text-st-done">{tokens(u.five_hours)}</span> · today <span className="text-text">{tokens(u.today)}</span> · 7 days{" "}
              <span className="text-text">{tokens(u.week)}</span>
            </span>
          )}
          {l?.installed && <span className="text-text-3">% after Claude's next reply</span>}
        </>
      )}
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
