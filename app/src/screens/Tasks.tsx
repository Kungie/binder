import { useEffect, useState } from "react";
import { Board } from "../components/Board";
import { TaskBoard } from "../components/TaskBoard";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";
import { matches, viewTasks } from "../lib/tasks";
import type { TaskView } from "../lib/types";

type Layout = "board" | "list";

const same = (a: TaskView, b: TaskView) => a.kind === b.kind && (a.kind !== "project" || (b.kind === "project" && a.id === b.id));
const layoutKey = (v: TaskView) => `binder.tasks.layout.${v.kind === "project" ? `project.${v.id}` : v.kind}`;

/** Board or list, remembered per view. Storage can be missing (private windows), so it falls back to the board. */
function readLayout(v: TaskView): Layout {
  try {
    return localStorage.getItem(layoutKey(v)) === "list" ? "list" : "board";
  } catch {
    return "board";
  }
}

export function Tasks() {
  const { tasks, view, setView, setCreatingTask } = useStore();
  // Project views live on the project page; Tasks opens on All.
  useEffect(() => {
    if (view.kind === "project") setView({ kind: "all" });
    // Only when Tasks opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const [showClosed, setShowClosed] = useState(false);
  const [query, setQuery] = useState("");
  const [layouts, setLayouts] = useState<Record<string, Layout>>({});
  const layout = layouts[layoutKey(view)] ?? readLayout(view);
  const choose = (l: Layout) => {
    setLayouts((m) => ({ ...m, [layoutKey(view)]: l }));
    try {
      localStorage.setItem(layoutKey(view), l);
    } catch {
      // Remembering is a nicety.
    }
  };
  const views: [TaskView, string][] = [
    [{ kind: "all" }, "All"],
    [{ kind: "mine" }, "Mine"],
    [{ kind: "agent" }, "Agent"],
  ];
  const found = (list: typeof tasks) => list.filter((t) => matches(t, query));
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-1 px-5" data-tauri-drag-region>
        <h1 className="mr-3 text-[13px] font-medium">Tasks</h1>
        <div role="group" aria-label="Task views" className="flex min-w-0 items-center gap-1 overflow-x-auto">
          {views.map(([v, label]) => (
            <button
              key={label}
              type="button"
              onClick={() => setView(v)}
              className={`h-6 shrink-0 rounded-control px-2 text-[12px] ${same(v, view) ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-2">
          <input
            type="search"
            aria-label="Search tasks"
            placeholder="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="h-7 w-44 rounded-control border border-line bg-card px-2 text-[12px] placeholder:text-text-3 focus:border-text-3 focus:outline-none"
          />
          <div className="flex h-7 items-center rounded-control border border-line bg-card p-0.5">
            {(["board", "list"] as const).map((l) => (
              <button
                key={l}
                type="button"
                aria-pressed={layout === l}
                onClick={() => choose(l)}
                className={`h-full rounded-[6px] px-2 text-[12px] ${layout === l ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
              >
                {l === "board" ? "Board" : "List"}
              </button>
            ))}
          </div>
          {layout === "list" && (
            <label className="flex items-center gap-2 text-[12px] text-text-3">
              <input type="checkbox" checked={showClosed} onChange={(e) => setShowClosed(e.target.checked)} className="accent-[#ededed]" />
              Show closed
            </label>
          )}
          <Button variant="primary" onClick={() => setCreatingTask(true)}>
            New task
          </Button>
        </div>
      </header>
      {layout === "board" ? (
        <Board tasks={found(viewTasks(tasks, view, true)).filter((t) => t.status !== "canceled")} />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto border-b border-line">
          <TaskBoard tasks={found(viewTasks(tasks, view, showClosed))} empty="No tasks in this view. Press c to add one." />
        </div>
      )}
    </div>
  );
}
