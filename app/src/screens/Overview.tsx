import { useEffect, useState } from "react";
import { Dot, Metric } from "../components/Metric";
import { Panel } from "../components/Panel";
import { TaskBoard } from "../components/TaskBoard";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";
import { viewTasks } from "../lib/tasks";
import type { DoneItem } from "../lib/types";
import { clock, useServer } from "../lib/useServer";
import { Progress } from "../components/Progress";
import { isPrinting } from "../lib/types";
import { minutes, usePrinters } from "../lib/usePrinters";
import { bytes } from "../lib/format";
import { ago } from "../lib/time";
import type { CloudEntry, CloudStatus } from "../lib/types";

const WEEK = 7 * 24 * 3600;

export function Overview() {
  const { api, tasks, setScreen, setCreatingTask, doneVersion } = useStore();
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

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Overview</h1>
        <span className="text-[12px] text-text-3">{today}</span>
        <Button variant="primary" className="ml-auto" onClick={() => setCreatingTask(true)}>
          New task
        </Button>
      </header>
      <div className="flex flex-col gap-3 px-5 pb-5">
        <div className="grid grid-cols-3 gap-3">
          <Metric title="Open" value={open.length} />
          <Metric title="In progress" value={doing} />
          <Metric title="Done this week" value={doneWeek} />
        </div>
        <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] items-start gap-3">
          <Panel
            title="Open tasks"
            action={
              <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("tasks")}>
                View all
              </button>
            }
          >
            <TaskBoard tasks={open.slice(0, 8)} compact label="Open tasks" empty="Nothing open. Press c to add a task." />
          </Panel>
          <div className="flex min-w-0 flex-col gap-3">
          <ServerPanel />
          <CloudPanel />
          <PrinterPanel />
          <DoneToday items={doneToday} />
          </div>
        </div>
      </div>
    </div>
  );
}

const DONE_SHOWN = 8;

/** Today's done lines across projects, newest first: the newest 8, then all of them on request. */
function DoneToday({ items }: { items: DoneItem[] }) {
  const [all, setAll] = useState(false);
  const shown = all ? items : items.slice(0, DONE_SHOWN);
  return (
    <Panel title="Done today" action={<span className="font-mono text-[11px] text-text-3">{items.length}</span>}>
      <ul aria-label="Done today" className="border-t border-line">
        {shown.map((d, i) => (
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
      {items.length === 0 && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">Nothing logged today.</p>}
      {items.length > DONE_SHOWN && (
        <button type="button" className="w-full border-t border-line px-3 py-2 text-left text-[12px] text-text-3 hover:text-text" onClick={() => setAll(!all)}>
          {all ? "Show fewer" : `Show all (${items.length})`}
        </button>
      )}
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
          space && (
            <>
              <span className="font-mono text-[11px] text-text-2">{`${bytes(space.free)} free of ${bytes(space.total)}`}</span>
              <Progress value={space.total ? ((space.total - space.free) / space.total) * 100 : 0} label="Cloud space used" />
            </>
          )
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
          {status && <span className="ml-auto font-mono text-[11px] text-text-3">load {status.load[0].toFixed(2)}</span>}
        </div>
        {status && <span className="text-text-2">{`${up} of ${status.containers.length} containers up`}</span>}
        {error && <span className="text-text-3">Can't reach it.{status ? ` Last read ${clock(status.at)}.` : ""}</span>}
      </div>
    </Panel>
  );
}

function PrinterPanel() {
  const { setScreen } = useStore();
  const { printers } = usePrinters();
  const p = printers.find((x) => isPrinting(x.state));
  if (!p) return null;
  return (
    <Panel
      label="Printer"
      title="Printer"
      action={
        <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={() => setScreen("printer")}>
          Open
        </button>
      }
    >
      <div className="flex flex-col gap-2 border-t border-line px-3 py-2.5 text-[12px]">
        <div className="flex items-center gap-2">
          <span className="min-w-0 truncate text-text">{p.state.job ?? p.config.name}</span>
          <span className="ml-auto font-mono text-[11px] text-text">{`${p.state.percent ?? 0}%`}</span>
        </div>
        <Progress value={p.state.percent ?? 0} label="Progress" />
        <span className="font-mono text-[11px] text-text-3">{`${minutes(p.state.remaining_min)} left`}</span>
      </div>
    </Panel>
  );
}
