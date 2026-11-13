import { useEffect, useRef, useState } from "react";
import { useStore } from "../lib/store";
import { PRIORITIES, PRIORITY_LABEL, STATUSES, STATUS_LABEL, type Task } from "../lib/types";
import { AgentRun } from "./AgentRun";
import { Button, Field, fieldClass } from "./ui";

/** The side sheet for one task. Text saves on blur, menus save on change. */
export function TaskSheet({ task, onClose, onStatus }: { task: Task; onClose(): void; onStatus(status: Task["status"]): void }) {
  const { projects, updateTask, deleteTask } = useStore();
  const [title, setTitle] = useState(task.title);
  const [notes, setNotes] = useState(task.notes);
  const [confirm, setConfirm] = useState(false);

  useEffect(() => {
    setTitle(task.title);
    setNotes(task.notes);
    setConfirm(false);
  }, [task.id, task.title, task.notes]);

  // Closing doesn't blur the fields, so text typed since the last blur is saved here.
  const latest = useRef({ title, notes, task });
  latest.current = { title, notes, task };
  const close = useRef(() => {});
  close.current = () => {
    const { title: t, notes: n, task: current } = latest.current;
    const patch: { title?: string; notes?: string } = {};
    if (t.trim() && t !== current.title) patch.title = t;
    if (n !== current.notes) patch.notes = n;
    if (patch.title !== undefined || patch.notes !== undefined) void updateTask(current.id, patch);
    onClose();
  };

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => e.key === "Escape" && close.current();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <aside role="dialog" aria-label={task.key} className="fade-in absolute inset-y-0 right-0 z-30 flex w-[380px] flex-col border-l border-line bg-panel shadow-menu">
      <header className="flex h-11 items-center gap-2 border-b border-line px-4">
        <span className="font-mono text-[11px] text-text-3">{task.key}</span>
        <Button variant="ghost" className="ml-auto" onClick={() => close.current()} aria-label="Close">
          Esc
        </Button>
      </header>
      <div className="flex flex-1 flex-col gap-3 overflow-y-auto p-4">
        <input
          aria-label="Title"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => title.trim() && title !== task.title && void updateTask(task.id, { title })}
          className="bg-transparent text-[15px] font-medium text-text focus:outline-none"
        />
        <Field label="Status">
          <select className={fieldClass} value={task.status} onChange={(e) => onStatus(e.target.value as Task["status"])}>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {STATUS_LABEL[s]}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Priority">
          <select className={fieldClass} value={task.priority} onChange={(e) => void updateTask(task.id, { priority: e.target.value as Task["priority"] })}>
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {PRIORITY_LABEL[p]}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Project">
          <select
            className={fieldClass}
            value={task.project_id ?? ""}
            onChange={(e) => void updateTask(task.id, e.target.value ? { project_id: Number(e.target.value) } : { clear_project: true })}
          >
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Assignee">
          <select className={fieldClass} value={task.assignee} onChange={(e) => void updateTask(task.id, { assignee: e.target.value as Task["assignee"] })}>
            <option value="me">Me</option>
            <option value="agent">Agent</option>
          </select>
        </Field>
        <Field label="Due">
          <input
            type="date"
            className={`${fieldClass} font-mono`}
            value={task.due ?? ""}
            onChange={(e) => void updateTask(task.id, e.target.value ? { due: e.target.value } : { clear_due: true })}
          />
        </Field>
        {task.assignee === "agent" && <AgentRun key={task.id} task={task} />}
        <textarea
          aria-label="Notes"
          placeholder="Notes, in Markdown"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          onBlur={() => notes !== task.notes && void updateTask(task.id, { notes })}
          className="min-h-40 flex-1 resize-none rounded-control border border-line bg-card p-2 font-mono text-[12px] leading-relaxed text-text placeholder:text-text-3 focus:border-text-3 focus:outline-none"
        />
      </div>
      <footer className="flex items-center border-t border-line px-4 py-3">
        {confirm ? (
          <>
            <span className="text-[12px] text-text-2">Delete {task.key}?</span>
            <Button variant="ghost" className="ml-auto" onClick={() => setConfirm(false)}>
              Keep
            </Button>
            <Button variant="secondary" onClick={() => void deleteTask(task.id).then(onClose)}>
              Delete
            </Button>
          </>
        ) : (
          <Button variant="ghost" onClick={() => setConfirm(true)}>
            Delete task
          </Button>
        )}
      </footer>
    </aside>
  );
}
