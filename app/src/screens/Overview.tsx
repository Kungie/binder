import { DiskBars, useSdSpace } from "../components/DiskBars";
import { useCallback, useEffect, useState } from "react";
import { DEFAULT_LAYOUT, GithubWidget, SitesWidget, clean } from "../components/OverviewWidgets";
import { EditBar, OverviewGrid } from "../components/OverviewGrid";
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
  // Escape ends editing, unless something open inside took it first.
  useEffect(() => {
    if (!editing) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && !e.defaultPrevented && setEditing(false);
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [editing]);
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
  // Bumps when the vault is connected from the notice, so the older lines come in.
  const [vaultTick, setVaultTick] = useState(0);
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
  }, [api, doneVersion, tasks, lastDone, vaultTick]);

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
        return <DoneToday items={doneToday} onConnected={() => setVaultTick((t) => t + 1)} />;
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
        <OverviewGrid layout={layout} editing={editing} save={save} render={widget} />
      </div>
    </div>
  );
}

/** Today's done lines across projects, newest first, scrolling inside the panel. */
function DoneToday({ items, onConnected }: { items: DoneItem[]; onConnected(): void }) {
  const { connected, connect, connecting, error } = useVault();
  return (
    <Panel label="Done today" className="flex min-h-0 flex-col" title="Done today" action={<span className="font-mono text-[11px] text-text-3">{items.length}</span>}>
      {connected === false && (
        <div className="flex flex-col gap-1 border-t border-line px-3 py-2 text-[12px] text-text-3">
          <span className="flex items-center gap-2">
            <span className="min-w-0 flex-1">Vault not connected. Older lines come back when it is.</span>
            <Button variant="ghost" className="shrink-0" disabled={connecting} onClick={() => void connect().then(onConnected)}>
              {connecting ? "Connecting…" : "Connect"}
            </Button>
          </span>
          {error && <span className="text-text-2">{error}</span>}
        </div>
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
