import type { DragEvent, KeyboardEvent } from "react";
import { dueLabel, localDay, plainText } from "../lib/tasks";
import { type Project, STATUS_LABEL, type Task } from "../lib/types";
import { Chip } from "./Chip";
import { ProjectDot } from "./ProjectDot";
import { StatusIcon } from "./StatusIcon";

const Calendar = () => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.1" aria-hidden="true">
    <rect x="1.5" y="2.5" width="9" height="8" rx="1.5" />
    <path d="M1.5 5h9M4 1.5v2M8 1.5v2" strokeLinecap="round" />
  </svg>
);

const Clock = () => (
  <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.1" aria-hidden="true">
    <circle cx="6" cy="6" r="4.5" />
    <path d="M6 3.5V6l1.6 1.2" strokeLinecap="round" />
  </svg>
);

/** A simple monochrome mark for the Claude agent: a six-point spark. */
export const ClaudeMark = () => (
  <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" className="text-[#D97757]">
    <path d="M6 1v10M1.7 3.5l8.6 5M1.7 8.5l8.6-5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
  </svg>
);

export interface BoardCardProps {
  task: Task;
  project: Project | undefined;
  focused: boolean;
  dropping: boolean;
  onOpen(): void;
  onFocus(): void;
  onKeyDown(e: KeyboardEvent): void;
  onDragStart(e: DragEvent): void;
  onDragEnd(): void;
  onDragOver(e: DragEvent): void;
  onDrop(e: DragEvent): void;
}

/** One soft card: the title, two lines of notes, then chips for status, due, project and the agent. */
export function BoardCard({ task, project, focused, dropping, onOpen, onFocus, onKeyDown, onDragStart, onDragEnd, onDragOver, onDrop }: BoardCardProps) {
  const notes = plainText(task.notes);
  const due = task.due ? dueLabel(task.due, task.due_time, localDay()) : null;
  return (
    <li className="relative">
      {dropping && <span aria-hidden="true" className="absolute -top-[5px] left-1 right-1 h-0.5 rounded-full bg-text-3" />}
      <div
        role="button"
        tabIndex={focused ? 0 : -1}
        data-id={task.id}
        data-key={task.key}
        aria-label={`${task.key} ${task.title}`}
        draggable
        onClick={onOpen}
        onFocus={onFocus}
        onKeyDown={onKeyDown}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        onDragOver={onDragOver}
        onDrop={onDrop}
        className="flex cursor-default flex-col gap-1.5 rounded-panel border border-line bg-card p-3 text-left transition-colors duration-[120ms] hover:border-line-strong hover:bg-raised focus:outline-none focus-visible:border-text-3"
      >
        <div className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-text">{task.title}</span>
          {task.attention && <span role="img" aria-label="Needs attention" className="size-1.5 shrink-0 rounded-full bg-urgent" />}
        </div>
        {notes && <p className="line-clamp-2 text-[12px] leading-[18px] text-text-2">{notes}</p>}
        <div className="mt-0.5 flex flex-wrap items-center gap-1">
          <Chip icon={<StatusIcon status={task.status} size={12} />}>{STATUS_LABEL[task.status]}</Chip>
          {due && (
            <Chip icon={task.due_time ? <Clock /> : <Calendar />} tone={due.overdue ? "warn" : "default"}>
              {due.text}
            </Chip>
          )}
          {project && <Chip icon={<ProjectDot color={project.color} />}>{project.name}</Chip>}
          {task.assignee === "agent" && <Chip icon={<ClaudeMark />}>Claude</Chip>}
        </div>
      </div>
    </li>
  );
}
