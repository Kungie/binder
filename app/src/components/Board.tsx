import { type DragEvent, type KeyboardEvent, useMemo, useRef, useState } from "react";
import { useStore } from "../lib/store";
import { STATUS_LABEL, type Status, type Task } from "../lib/types";
import { BoardCard } from "./BoardCard";
import { StatusIcon } from "./StatusIcon";
import { useTaskFlow } from "./TaskBoard";

const COLUMNS: Status[] = ["backlog", "todo", "in_progress", "done"];
const WEEK = 7 * 86400;
/** Spacing used when a column is renumbered, so later drops find a gap. */
const STEP = 1024;

const ordered = (tasks: Task[]) => [...tasks].sort((a, b) => a.sort - b.sort || a.id - b.id);

/** Roma-style columns of cards by status. Drag between columns to change status, within one to reorder. */
export function Board({ tasks }: { tasks: Task[] }) {
  const store = useStore();
  const flow = useTaskFlow();
  const [showAllDone, setShowAllDone] = useState(false);
  const [focusId, setFocusId] = useState<number | null>(null);
  const [over, setOver] = useState<{ status: Status; before: number | null } | null>(null);
  const dragging = useRef<number | null>(null);
  const root = useRef<HTMLDivElement>(null);

  const byColumn = useMemo(() => Object.fromEntries(COLUMNS.map((s) => [s, ordered(tasks.filter((t) => t.status === s))])) as Record<Status, Task[]>, [tasks]);
  const since = Date.now() / 1000 - WEEK;
  const shown = (s: Status) => (s === "done" && !showAllDone ? byColumn.done.filter((t) => t.updated >= since) : byColumn[s]);
  const olderDone = byColumn.done.length - shown("done").length;
  const firstId = COLUMNS.flatMap(shown)[0]?.id;

  /** Puts a task before `beforeId` (or at the end) of a column: the midpoint of its neighbours, or a renumbered column. */
  const place = async (id: number, status: Status, beforeId: number | null) => {
    const task = store.tasks.find((t) => t.id === id);
    if (!task || beforeId === id) return;
    const list = (byColumn[status] ?? []).filter((t) => t.id !== id);
    let at = beforeId === null ? list.length : list.findIndex((t) => t.id === beforeId);
    if (at < 0) at = list.length;
    const prev = list[at - 1]?.sort;
    const next = list[at]?.sort;
    let sort: number;
    if (prev === undefined && next === undefined) sort = task.sort;
    else if (prev === undefined) sort = next! - 1;
    else if (next === undefined) sort = prev + 1;
    else if (next - prev > 1) sort = Math.floor((prev + next) / 2);
    else {
      const order = [...list];
      order.splice(at, 0, task);
      const moves = order.map((t, k) => [t, (k + 1) * STEP] as const).filter(([t, s]) => t.id !== id && t.sort !== s);
      await Promise.all(moves.map(([t, s]) => store.api.updateTask(t.id, { sort: s })));
      sort = (at + 1) * STEP;
    }
    if (status !== task.status) await flow.setStatus([id], status, { sort });
    else if (sort !== task.sort) await store.updateTask(id, { sort });
    else await store.reload();
  };

  const dropped = (e: DragEvent, status: Status, beforeId: number | null) => {
    e.preventDefault();
    e.stopPropagation();
    const id = dragging.current ?? Number(e.dataTransfer.getData("text/plain"));
    dragging.current = null;
    setOver(null);
    if (id) void place(id, status, beforeId);
  };

  const focusCard = (t: Task | undefined) => {
    if (!t) return;
    setFocusId(t.id);
    root.current?.querySelector<HTMLElement>(`[data-id="${t.id}"]`)?.focus();
  };

  const onKey = (e: KeyboardEvent, status: Status, index: number, task: Task) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const col = COLUMNS.indexOf(status);
    const sideways = (dir: number) => {
      for (let c = col + dir; c >= 0 && c < COLUMNS.length; c += dir) {
        const list = shown(COLUMNS[c]);
        if (list.length) return focusCard(list[Math.min(index, list.length - 1)]);
      }
    };
    const keys: Record<string, () => void> = {
      ArrowDown: () => focusCard(shown(status)[index + 1]),
      ArrowUp: () => focusCard(shown(status)[index - 1]),
      ArrowLeft: () => sideways(-1),
      ArrowRight: () => sideways(1),
      Enter: () => flow.open(task.id),
      c: () => store.setCreatingTask(true, status),
    };
    keys.j = keys.ArrowDown;
    keys.k = keys.ArrowUp;
    keys.h = keys.ArrowLeft;
    keys.l = keys.ArrowRight;
    const run = keys[e.key];
    if (!run) return;
    e.preventDefault();
    e.stopPropagation();
    run();
  };

  return (
    <div ref={root} className="flex min-h-0 flex-1 gap-3 overflow-x-auto p-5 pt-1">
      {COLUMNS.map((status) => {
        const list = shown(status);
        const label = STATUS_LABEL[status];
        return (
          <section
            key={status}
            aria-label={label}
            onDragOver={(e) => {
              e.preventDefault();
              if (over?.status !== status || over.before !== null) setOver({ status, before: null });
            }}
            onDrop={(e) => dropped(e, status, null)}
            className={`flex w-[288px] shrink-0 flex-col rounded-panel bg-panel ${over?.status === status ? "ring-1 ring-line-strong" : ""}`}
          >
            <header className="flex h-10 shrink-0 items-center gap-2 px-3">
              <StatusIcon status={status} />
              <h2 className="text-[12px] font-medium text-text">{label}</h2>
              <span aria-label={list.length === 1 ? "1 task" : `${list.length} tasks`} className="font-mono text-[11px] text-text-3">
                {list.length}
              </span>
              <button
                type="button"
                aria-label={`New ${label} task`}
                onClick={() => store.setCreatingTask(true, status)}
                className="ml-auto grid size-6 place-items-center rounded-control text-text-3 hover:bg-raised hover:text-text"
              >
                <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
                  <path d="M6 2v8M2 6h8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
                </svg>
              </button>
            </header>
            <ul className="flex min-h-[48px] flex-1 flex-col gap-2 overflow-y-auto px-2 pb-2">
              {list.map((task, index) => (
                <BoardCard
                  key={task.id}
                  task={task}
                  project={store.projects.find((p) => p.id === task.project_id)}
                  focused={(focusId ?? firstId) === task.id}
                  dropping={over?.status === status && over.before === task.id}
                  onOpen={() => flow.open(task.id)}
                  onFocus={() => setFocusId(task.id)}
                  onKeyDown={(e) => onKey(e, status, index, task)}
                  onDragStart={(e) => {
                    dragging.current = task.id;
                    e.dataTransfer.setData("text/plain", String(task.id));
                    e.dataTransfer.effectAllowed = "move";
                  }}
                  onDragEnd={() => {
                    dragging.current = null;
                    setOver(null);
                  }}
                  onDragOver={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    if (over?.status !== status || over.before !== task.id) setOver({ status, before: task.id });
                  }}
                  onDrop={(e) => dropped(e, status, task.id)}
                />
              ))}
            </ul>
            {status === "done" && (olderDone > 0 || showAllDone) && (
              <button type="button" onClick={() => setShowAllDone((v) => !v)} className="h-8 shrink-0 border-t border-line text-[11px] text-text-3 hover:text-text-2">
                {showAllDone ? "Last 7 days" : "Show all"}
              </button>
            )}
          </section>
        );
      })}
      {flow.overlays}
    </div>
  );
}
