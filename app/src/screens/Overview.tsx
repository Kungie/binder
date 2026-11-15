import { DiskBars, useSdSpace } from "../components/DiskBars";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { DEFAULT_LAYOUT, GithubWidget, SitesWidget, WIDGETS, FILLS, clean, move } from "../components/OverviewWidgets";
import { Dot, Metric } from "../components/Metric";
import { Panel } from "../components/Panel";
import { TaskBoard } from "../components/TaskBoard";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";
import { viewTasks } from "../lib/tasks";
import type { DoneItem, OverviewLayout } from "../lib/types";
import { clock, useServer } from "../lib/useServer";
import { Progress } from "../components/Progress";
import { isPrinting } from "../lib/types";
import { Brief } from "../components/Brief";
import { minutes, STATE_LABEL, temp, usePrinters } from "../lib/usePrinters";
import { ago } from "../lib/time";
import { useVault } from "../lib/useVault";
import { loadPercent } from "../lib/load";
import type { CloudEntry, CloudStatus } from "../lib/types";
import { Empty } from "../components/Mascot";

const WEEK = 7 * 24 * 3600;

export function Overview() {
  const { api, tasks, setScreen, setCreatingTask, doneVersion } = useStore();
  const [layout, setLayout] = useState<OverviewLayout>(DEFAULT_LAYOUT);
  const [editing, setEditing] = useState(false);
  useEffect(() => {
    let live = true;
    void api.getConfig().then((c) => live && c.overview && setLayout(clean(c.overview)), () => {});
    return () => {
      live = false;
    };
  }, [api]);
  const save = useCallback(
    (next: OverviewLayout) => {
      const l = clean(next);
      setLayout(l);
      void api.getConfig().then((c) => api.setConfig({ ...c, overview: l }));
    },
    [api],
  );
  const [doneToday, setDoneToday] = useState<DoneItem[]>([]);
  // Bumps when a hook writes a line in the background, so Done today refetches.
  const [lastDone, setLastDone] = useState<number | null>(null);
  const now = Math.floor(Date.now() / 1000);
  const open = viewTasks(tasks, { kind: "all" }, false);
  const doing = open.filter((t) => t.status === "in_progress").length;
  const doneWeek = tasks.filter((t) => t.status === "done" && now - t.updated <= WEEK).length;
  const today = new Date().toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" });

  useEffect(() => {
    let live = true;
    void api.doneToday().then((d) => live && setDoneToday(d));
    return () => {
      live = false;
    };
  }, [api, doneVersion, tasks, lastDone]);

  useEffect(() => {
    const check = () => void api.lastDone().then(setLastDone, () => {});
    check();
    const timer = window.setInterval(check, 30_000);
    return () => window.clearInterval(timer);
  }, [api]);

  const widget = (id: string) => {
    switch (id) {
      case "numbers":
        return (
          <div className="grid grid-cols-3 gap-3">
            <Metric title="Open" value={open.length} />
            <Metric title="In progress" value={doing} />
            <Metric title="Done this week" value={doneWeek} />
          </div>
        );
      case "done_today":
        return <DoneToday items={doneToday} />;
      case "server":
        return <ServerPanel />;
      case "cloud":
        return <CloudPanel />;
      case "printer":
        return <PrinterPanel />;
      case "sites":
        return <SitesWidget />;
      case "github":
        return <GithubWidget />;
      case "open_tasks":
        return (
          <Panel
            className="flex min-h-[180px] flex-1 flex-col"
            title="Open tasks"
            action={
              <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("tasks")}>
                View all
              </button>
            }
          >
            <div className="min-h-0 flex-1 overflow-y-auto">
              <TaskBoard tasks={open} compact label="Open tasks" empty="Nothing open. Press c to add a task." />
            </div>
          </Panel>
        );
      default:
        return null;
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Overview</h1>
        <span className="text-[12px] text-text-3">{today}</span>
        <span className="ml-auto flex items-center gap-2">
          {editing && <EditBar layout={layout} save={save} />}
          <Button variant="ghost" onClick={() => setEditing((e) => !e)}>
            {editing ? "Done" : "Edit"}
          </Button>
          <Button variant="primary" onClick={() => setCreatingTask(true)}>
            New task
          </Button>
        </span>
      </header>
      {/* The two columns fill the window and their long lists scroll inside; below a usable height the page scrolls. */}
      <div className="flex min-h-[620px] flex-1 flex-col gap-3 px-5 pb-5">
        <Brief />
        <Columns layout={layout} editing={editing} onMove={(id, side, before) => save(move(layout, id, side, before))} onHide={(id) => save({ left: layout.left.filter((x) => x !== id), right: layout.right.filter((x) => x !== id) })} render={widget} />
      </div>
    </div>
  );
}

/** Add widget and Reset, shown while editing. */
function EditBar({ layout, save }: { layout: OverviewLayout; save(l: OverviewLayout): void }) {
  const [menu, setMenu] = useState(false);
  const shown = new Set([...layout.left, ...layout.right]);
  const hidden = Object.keys(WIDGETS).filter((id) => !shown.has(id));
  return (
    <>
      <div className="relative">
        <Button variant="ghost" disabled={hidden.length === 0} onClick={() => setMenu((m) => !m)}>
          Add widget
        </Button>
        {menu && (
          <div role="menu" aria-label="Add widget" className="pop-in absolute right-0 top-8 z-20 flex min-w-40 flex-col rounded-control bg-raised p-1 shadow-menu">
            {hidden.map((id) => (
              <button
                key={id}
                type="button"
                role="menuitem"
                className="h-7 rounded-[6px] px-2 text-left text-[12px] text-text-2 hover:bg-card hover:text-text"
                onClick={() => {
                  setMenu(false);
                  const side = DEFAULT_LAYOUT.left.includes(id) ? "left" : DEFAULT_LAYOUT.right.includes(id) ? "right" : layout.left.length <= layout.right.length ? "left" : "right";
                  save({ ...layout, [side]: [...layout[side], id] });
                }}
              >
                {WIDGETS[id]}
              </button>
            ))}
          </div>
        )}
      </div>
      <Button variant="ghost" onClick={() => save(DEFAULT_LAYOUT)}>
        Reset
      </Button>
    </>
  );
}

type Over = { side: "left" | "right"; before: string | null };

/** The two columns. While editing, a widget's handle drags it; pointer events, since Tauri swallows HTML5 drags. */
function Columns({
  layout,
  editing,
  onMove,
  onHide,
  render,
}: {
  layout: OverviewLayout;
  editing: boolean;
  onMove(id: string, side: "left" | "right", before: string | null): void;
  onHide(id: string): void;
  render(id: string): ReactNode;
}) {
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<Over | null>(null);
  const latest = useRef<{ dragging: string | null; over: Over | null }>({ dragging: null, over: null });
  latest.current = { dragging, over };

  useEffect(() => {
    if (!dragging) return;
    const target = (x: number, y: number): Over | null => {
      const el = document.elementFromPoint(x, y);
      const w = el?.closest("[data-widget]");
      const col = el?.closest("[data-column]");
      if (!col) return null;
      const side = col.getAttribute("data-column") as "left" | "right";
      const id = w?.getAttribute("data-widget") ?? null;
      return { side, before: id === dragging ? null : id };
    };
    const onMoveEv = (e: PointerEvent | MouseEvent) => setOver(target(e.clientX, e.clientY));
    const onUp = () => {
      const { dragging: d, over: o } = latest.current;
      if (d && o) onMove(d, o.side, o.before);
      setDragging(null);
      setOver(null);
    };
    document.addEventListener("pointermove", onMoveEv);
    document.addEventListener("pointerup", onUp);
    return () => {
      document.removeEventListener("pointermove", onMoveEv);
      document.removeEventListener("pointerup", onUp);
    };
  }, [dragging, onMove]);

  const column = (side: "left" | "right") => (
    <div data-column={side} className="flex min-h-0 min-w-0 flex-col gap-3">
      {layout[side].map((id) => (
        <div key={id} data-widget={id} className={`relative ${FILLS.has(id) ? "flex min-h-[180px] flex-1 flex-col [&>*:last-child]:min-h-0 [&>*:last-child]:flex-1" : "shrink-0"} ${dragging === id ? "opacity-40" : ""}`}>
          {over?.side === side && over.before === id && dragging !== id && <div aria-hidden="true" className="absolute -top-2 left-0 right-0 h-0.5 rounded-full bg-text" />}
          {editing && (
            <div className="flex h-7 shrink-0 items-center gap-1 text-[11px] text-text-3">
              <button
                type="button"
                aria-label={`Move ${WIDGETS[id]}`}
                onPointerDown={(e) => {
                  e.preventDefault();
                  setDragging(id);
                }}
                className="flex h-6 cursor-grab items-center gap-1.5 rounded-[6px] pl-1 pr-2 hover:bg-card hover:text-text"
              >
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
                  {[3, 6, 9].flatMap((y) => [4, 8].map((x) => <circle key={`${x}-${y}`} cx={x} cy={y} r="1" />))}
                </svg>
                {WIDGETS[id]}
              </button>
              <button type="button" aria-label={`Hide ${WIDGETS[id]}`} onClick={() => onHide(id)} className="ml-auto h-6 rounded-[6px] px-2 hover:bg-card hover:text-text">
                Hide
              </button>
            </div>
          )}
          {render(id)}
        </div>
      ))}
      {over?.side === side && over.before === null && <div aria-hidden="true" className="h-0.5 shrink-0 rounded-full bg-text" />}
      {editing && <div className="h-12 shrink-0 rounded-panel border border-dashed border-line" aria-hidden="true" />}
    </div>
  );

  return (
    <div className={`grid min-h-0 flex-1 grid-cols-2 gap-3 ${dragging ? "select-none" : ""}`}>
      {column("left")}
      {column("right")}
    </div>
  );
}

/** Today's done lines across projects, newest first, scrolling inside the panel. */
function DoneToday({ items }: { items: DoneItem[] }) {
  const { connected } = useVault();
  return (
    <Panel label="Done today" className="flex min-h-0 flex-col" title="Done today" action={<span className="font-mono text-[11px] text-text-3">{items.length}</span>}>
      {connected === false && (
        <p className="border-t border-line px-3 py-2 text-[12px] text-text-3">Vault not connected. Older lines come back when it is.</p>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto">
        <ul aria-label="Done today" className="border-t border-line">
          {items.map((d, i) => (
            <li key={`${d.project_id}-${i}`} className="flex flex-col gap-0.5 border-b border-line px-3 py-2 text-[12px] last:border-b-0">
              <span className="flex items-center gap-2 text-text-3">
                {d.line.time && <span className="font-mono text-[11px]">{d.line.time}</span>}
                <span data-dot className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: d.color }} aria-hidden="true" />
                <span className="truncate">{d.project}</span>
              </span>
              <span className="text-text-2">{d.line.text}</span>
            </li>
          ))}
        </ul>
        {items.length === 0 && <Empty>Nothing logged today.</Empty>}
      </div>
    </Panel>
  );
}

/** Space on the share and the three most recently changed items at its root. */
function CloudPanel() {
  const { api, setScreen } = useStore();
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [recent, setRecent] = useState<CloudEntry[]>([]);
  useEffect(() => {
    let live = true;
    void api.cloudStatus().then(async (s) => {
      if (!live) return;
      setStatus(s);
      if (!s.mounted) return;
      const list = await api.cloudList("", false).catch(() => []);
      if (live) setRecent([...list].sort((a, b) => b.modified - a.modified).slice(0, 3));
    });
    return () => {
      live = false;
    };
  }, [api]);
  const space = status?.space;
  const sd = useSdSpace(status?.mounted ?? false);
  return (
    <Panel
      title="Cloud"
      label="Cloud"
      action={
        <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("cloud")}>
          Open
        </button>
      }
    >
      <div className="flex flex-col gap-2 border-t border-line px-3 py-2.5 text-[12px]">
        {!status?.mounted ? (
          <span className="text-text-3">Not connected</span>
        ) : (
          <DiskBars ssd={space ?? null} sd={sd} />
        )}
      </div>
      {recent.length > 0 && (
        <ul className="border-t border-line">
          {recent.map((e) => (
            <li key={e.rel} className="flex items-center gap-2 border-b border-line px-3 py-1.5 text-[12px] last:border-b-0">
              <span className="min-w-0 flex-1 truncate text-text-2">{e.name}</span>
              <span className="shrink-0 text-[11px] text-text-3">{ago(e.modified)}</span>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

function ServerPanel() {
  const { setScreen } = useStore();
  const { target, status, error } = useServer(30_000);
  if (!target) return null;
  const up = status?.containers.filter((c) => c.state === "running").length ?? 0;
  return (
    <Panel
      title="Server"
      action={
        <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("server")}>
          Open
        </button>
      }
    >
      <div className="flex flex-col gap-1.5 border-t border-line px-3 py-2.5 text-[12px]">
        <div className="flex items-center gap-2">
          <Dot on={!error && !!status} />
          <span className="font-mono text-[11px] text-text-2">{target}</span>
          {status && <span className="ml-auto font-mono text-[11px] text-text-3">load {loadPercent(status.load[0], status.cores) ?? status.load[0].toFixed(2)}</span>}
        </div>
        {status && <span className="text-text-2">{`${up} of ${status.containers.length} containers up`}</span>}
        {error && <span className="text-text-3">Can't reach it.{status ? ` Last read ${clock(status.at)}.` : ""}</span>}
      </div>
    </Panel>
  );
}

function PrinterPanel() {
  const { setScreen } = useStore();
  const { printers, loaded } = usePrinters();
  const p = printers.find((x) => isPrinting(x.state)) ?? printers[0];
  return (
    <Panel
      label="Printer"
      title="Printer"
      action={
        p && (
          <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("printer")}>
            Open
          </button>
        )
      }
    >
      {p && isPrinting(p.state) ? (
        <div className="flex flex-col gap-2 border-t border-line px-3 py-2.5 text-[12px]">
          <div className="flex items-center gap-2">
            <span className="min-w-0 truncate text-text">{p.state.job ?? p.config.name}</span>
            <span className="ml-auto font-mono text-[11px] text-text">{`${p.state.percent ?? 0}%`}</span>
          </div>
          <Progress value={p.state.percent ?? 0} label="Progress" />
          <span className="font-mono text-[11px] text-text-3">{`${minutes(p.state.remaining_min)} left`}</span>
        </div>
      ) : p ? (
        <div className="flex flex-col gap-1.5 border-t border-line px-3 py-2.5 text-[12px]">
          <div className="flex items-center gap-2">
            <Dot on={p.online} />
            <span className="min-w-0 truncate text-text">{p.config.name}</span>
            <span className="ml-auto text-text-3">{p.online ? (STATE_LABEL[p.state.gcode_state ?? ""] ?? "Idle") : "Offline"}</span>
          </div>
          {p.online && (
            <div className="flex gap-4 font-mono text-[11px] text-text-3">
              <span>
                Nozzle <span className="text-text-2">{temp(p.state.nozzle_c, p.state.nozzle_target_c)}</span>
              </span>
              <span>
                Bed <span className="text-text-2">{temp(p.state.bed_c, p.state.bed_target_c)}</span>
              </span>
            </div>
          )}
        </div>
      ) : (
        loaded && (
          <Empty
            action={
              <Button onClick={() => setScreen("printer")}>Set up</Button>
            }
          >
            No printer set up.
          </Empty>
        )
      )}
    </Panel>
  );
}
