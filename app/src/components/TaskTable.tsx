import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { type Assignee, type Project, STATUSES, type Status, type Task, isClosed } from "../lib/types";
import { PriorityIcon } from "./PriorityIcon";
import { StatusIcon } from "./StatusIcon";
import { StatusMenu } from "./StatusMenu";

export interface TaskTableProps {
  tasks: Task[];
  projects: Project[];
  onOpen(id: number): void;
  onCreate(): void;
  onStatus(ids: number[], status: Status): void;
  onAssign(id: number, assignee: Assignee): void;
  compact?: boolean;
  label?: string;
  empty?: string;
}

const Robot = () => (
  <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" aria-hidden="true">
    <rect x="2.5" y="4.5" width="9" height="7" rx="2" />
    <path d="M7 2v2.5M5.2 8h.01M8.8 8h.01" strokeLinecap="round" />
  </svg>
);

function shortDate(day: string) {
  const d = new Date(`${day}T00:00:00`);
  return d.toLocaleDateString("en-GB", { day: "numeric", month: "short" });
}

/** The Linear-style task table. Keys: j/k move, Enter opens, c creates, s status, a assignee, x select. */
export function TaskTable({ tasks, projects, onOpen, onCreate, onStatus, onAssign, compact = false, label = "Tasks", empty = "No tasks here." }: TaskTableProps) {
  const [focusId, setFocusId] = useState<number | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [menu, setMenu] = useState<number | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const names = useMemo(() => new Map(projects.map((p) => [p.id, p.name])), [projects]);

  const index = Math.max(0, tasks.findIndex((t) => t.id === focusId));
  const focused = tasks[index];

  useEffect(() => {
    gridRef.current?.querySelector(`[data-id="${focused?.id}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [focused?.id]);

  // Only rows still shown count: a view change must not act on tasks that left it.
  const shown = [...selected].filter((id) => tasks.some((t) => t.id === id));
  const targets = () => (shown.length > 0 ? shown : focused ? [focused.id] : []);
  const pick = (status: Status) => {
    const ids = targets();
    setMenu(null);
    setSelected(new Set());
    if (ids.length) onStatus(ids, status);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (menu !== null) {
      handled();
      if (/^[1-5]$/.test(e.key)) pick(STATUSES[Number(e.key) - 1]);
      else if (e.key === "ArrowDown" || e.key === "j") setMenu((m) => Math.min(STATUSES.length - 1, (m ?? 0) + 1));
      else if (e.key === "ArrowUp" || e.key === "k") setMenu((m) => Math.max(0, (m ?? 0) - 1));
      else if (e.key === "Enter") pick(STATUSES[menu]);
      else if (e.key === "Escape") setMenu(null);
      return;
    }
    const move = (d: number) => tasks.length && setFocusId(tasks[Math.min(tasks.length - 1, Math.max(0, index + d))].id);
    switch (e.key) {
      case "j":
      case "ArrowDown":
        handled();
        move(1);
        break;
      case "k":
      case "ArrowUp":
        handled();
        move(-1);
        break;
      case "Enter":
        if (focused) {
          handled();
          onOpen(focused.id);
        }
        break;
      case "c":
        handled();
        onCreate();
        break;
      case "s":
        if (focused) {
          handled();
          setMenu(STATUSES.indexOf(focused.status));
        }
        break;
      case "a":
        if (focused) {
          handled();
          onAssign(focused.id, focused.assignee === "me" ? "agent" : "me");
        }
        break;
      case "x":
        if (focused) {
          handled();
          setSelected((s) => {
            const next = new Set(s);
            if (next.has(focused.id)) next.delete(focused.id);
            else next.add(focused.id);
            return next;
          });
        }
        break;
      case "Escape":
        if (selected.size) {
          handled();
          setSelected(new Set());
        }
        break;
    }
  };

  const cols = compact ? "grid-cols-[16px_60px_minmax(0,1fr)_92px_64px]" : "grid-cols-[16px_60px_minmax(0,1fr)_104px_64px_16px_56px]";

  return (
    <div
      ref={gridRef}
      role="grid"
      aria-label={label}
      aria-multiselectable="true"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setMenu(null);
      }}
      className="relative outline-none focus-visible:outline-none"
    >
      {tasks.length === 0 && <p className="px-3 py-6 text-[12px] text-text-3">{empty}</p>}
      {tasks.map((t, i) => {
        const isFocused = i === index;
        const isSelected = shown.includes(t.id);
        const closed = isClosed(t.status);
        return (
          <div key={t.id} className="relative">
            <div
              role="row"
              data-id={t.id}
              data-key={t.key}
              aria-selected={isFocused}
              data-selected={isSelected || undefined}
              onMouseDown={() => setFocusId(t.id)}
              onDoubleClick={() => onOpen(t.id)}
              className={`grid h-9 ${cols} items-center gap-3 border-t border-line px-3 text-[12px] ${
                isFocused ? "bg-raised" : "hover:bg-raised/50"
              } ${isSelected ? "shadow-[inset_2px_0_0_var(--color-text)]" : ""}`}
            >
              <span role="gridcell">
                <StatusIcon status={t.status} />
              </span>
              <span role="gridcell" className="font-mono text-[11px] text-text-3">
                {t.key}
              </span>
              <span role="gridcell" className={`truncate ${closed ? "text-text-3 line-through" : "text-text"}`} onClick={() => onOpen(t.id)}>
                {t.title}
              </span>
              <span role="gridcell" className="truncate text-text-3">
                {t.project_id ? names.get(t.project_id) : ""}
              </span>
              <span role="gridcell" className={`flex items-center gap-1 ${t.assignee === "agent" ? "text-text" : "text-text-3"}`}>
                {t.assignee === "agent" && <Robot />}
                {t.assignee}
              </span>
              {!compact && (
                <>
                  <span role="gridcell">
                    <PriorityIcon priority={t.priority} />
                  </span>
                  <span role="gridcell" className="text-right font-mono text-[11px] text-text-3">
                    {t.due ? shortDate(t.due) : ""}
                  </span>
                </>
              )}
            </div>
            {isFocused && menu !== null && (
              <div className="absolute right-0 top-9">
                <StatusMenu active={menu} onPick={pick} />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
