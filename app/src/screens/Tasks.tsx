import { useState } from "react";
import { TaskBoard } from "../components/TaskBoard";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";
import { viewTasks } from "../lib/tasks";
import type { TaskView } from "../lib/types";

const same = (a: TaskView, b: TaskView) => a.kind === b.kind && (a.kind !== "project" || (b.kind === "project" && a.id === b.id));

export function Tasks() {
  const { tasks, projects, view, setView, setCreatingTask } = useStore();
  const [showClosed, setShowClosed] = useState(false);
  const views: [TaskView, string][] = [
    [{ kind: "all" }, "All"],
    [{ kind: "mine" }, "Mine"],
    [{ kind: "agent" }, "Agent"],
    ...projects.filter((p) => p.state === "active").map((p): [TaskView, string] => [{ kind: "project", id: p.id }, p.name]),
  ];
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-1 px-5" data-tauri-drag-region>
        <h1 className="mr-3 text-[13px] font-medium">Tasks</h1>
        {views.map(([v, label]) => (
          <button
            key={label}
            type="button"
            onClick={() => setView(v)}
            className={`h-6 rounded-control px-2 text-[12px] ${same(v, view) ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
          >
            {label}
          </button>
        ))}
        <label className="ml-auto mr-3 flex items-center gap-2 text-[12px] text-text-3">
          <input type="checkbox" checked={showClosed} onChange={(e) => setShowClosed(e.target.checked)} className="accent-[#ededed]" />
          Show closed
        </label>
        <Button variant="primary" onClick={() => setCreatingTask(true)}>
          New task
        </Button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto border-b border-line">
        <TaskBoard tasks={viewTasks(tasks, view, showClosed)} empty="No tasks in this view. Press c to add one." />
      </div>
    </div>
  );
}
