import { useEffect, useState } from "react";
import { Kpi } from "../components/panelui/kpi";
import { Panel } from "../components/Panel";
import { TaskBoard } from "../components/TaskBoard";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";
import { viewTasks } from "../lib/tasks";
import type { DoneLine } from "../lib/types";

const WEEK = 7 * 24 * 3600;

function Metric({ title, value }: { title: string; value: number }) {
  return (
    <Kpi surface role="group" aria-label={`${title}: ${value}`} className="!gap-1 !p-4">
      <Kpi.Header>
        <Kpi.Title className="!text-[12px] !font-normal">{title}</Kpi.Title>
      </Kpi.Header>
      <Kpi.Value className="font-mono !text-[24px] !font-normal">{value}</Kpi.Value>
    </Kpi>
  );
}

export function Overview() {
  const { api, tasks, setScreen, setCreatingTask, doneVersion } = useStore();
  const [doneToday, setDoneToday] = useState<[string, DoneLine][]>([]);
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
  }, [api, doneVersion, tasks]);

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
          <Panel title="Done today" action={<span className="font-mono text-[11px] text-text-3">{doneToday.length}</span>}>
            <ul className="border-t border-line">
              {doneToday.map(([project, line], i) => (
                <li key={i} className="flex flex-col gap-0.5 border-b border-line px-3 py-2 text-[12px] last:border-b-0">
                  <span className="text-text-3">{project}</span>
                  <span className="text-text-2">{line.text}</span>
                </li>
              ))}
              {doneToday.length === 0 && <li className="px-3 py-3 text-[12px] text-text-3">Nothing logged today.</li>}
            </ul>
          </Panel>
        </div>
      </div>
    </div>
  );
}
