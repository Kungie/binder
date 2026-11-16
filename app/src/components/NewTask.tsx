import { useState } from "react";
import { useStore } from "../lib/store";
import { type Assignee, type Priority, type Repeat, STATUSES, STATUS_LABEL, type Status } from "../lib/types";
import { REPEATS } from "../lib/repeat";
import { Moon } from "./Moon";
import { PRIORITY_SHORT, PriorityChip } from "./PriorityIcon";
import { PriorityMenu } from "./PriorityMenu";
import { Button, Switch, fieldClass } from "./ui";

/** The sheet's field look, sized to its content so the controls share a row. */
const chip = fieldClass.replace("w-full", "w-auto");

/** The new-task dialog: everything the task sheet sets, then Create task. Enter in the title or ⌘Enter creates, Esc cancels. */
export function NewTask() {
  const { projects, view, createTask, setCreatingTask, screen, selectedProject, newTaskStatus } = useStore();
  const preset = view.kind === "project" ? view.id : screen === "projects" ? selectedProject : null;
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [status, setStatus] = useState<Status>(newTaskStatus ?? "todo");
  const [projectId, setProjectId] = useState<number | "">(preset ?? "");
  const [priority, setPriority] = useState<Priority>("none");
  const [prioMenu, setPrioMenu] = useState(false);
  const [assignee, setAssignee] = useState<Assignee>(view.kind === "agent" || view.kind === "night" ? "agent" : "me");
  const [night, setNight] = useState(view.kind === "night");
  const [due, setDue] = useState("");
  const [dueTime, setDueTime] = useState("");
  const [repeat, setRepeat] = useState<Repeat | "">("");
  const [error, setError] = useState<string | null>(null);
  const close = () => setCreatingTask(false);

  const submit = async () => {
    if (!title.trim()) return setError("Give it a title.");
    try {
      await createTask({
        title,
        notes: notes || undefined,
        status,
        priority,
        project_id: projectId === "" ? undefined : projectId,
        assignee,
        night,
        due: due || undefined,
        due_time: due && dueTime ? dueTime : undefined,
        repeat: repeat || undefined,
      });
      close();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="backdrop-in fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[14vh]" onMouseDown={close}>
      <div
        role="dialog"
        aria-label="New task"
        className="pop-in w-[560px] max-w-[calc(100vw-32px)] rounded-panel border border-line-strong bg-panel p-4 shadow-menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") return close();
          const inNotes = (e.target as HTMLElement).tagName === "TEXTAREA";
          if (e.key === "Enter" && (e.metaKey || (!e.shiftKey && !inNotes && (e.target as HTMLElement).tagName === "INPUT"))) {
            e.preventDefault();
            void submit();
          }
        }}
      >
        <input
          autoFocus
          aria-label="Task title"
          placeholder="Task title"
          value={title}
          onChange={(e) => {
            setTitle(e.target.value);
            setError(null);
          }}
          className="w-full bg-transparent text-[15px] text-text placeholder:text-text-3 focus:outline-none"
        />
        <textarea
          aria-label="Notes"
          placeholder="Add notes"
          rows={2}
          value={notes}
          onChange={(e) => {
            setNotes(e.target.value);
            e.target.style.height = "auto";
            e.target.style.height = `${e.target.scrollHeight}px`;
          }}
          className="mt-2 max-h-[40vh] min-h-10 w-full resize-none bg-transparent text-[13px] text-text-2 placeholder:text-text-3 focus:outline-none"
        />
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <select aria-label="Status" className={chip} value={status} onChange={(e) => setStatus(e.target.value as Status)}>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {STATUS_LABEL[s]}
              </option>
            ))}
          </select>
          <div className="relative">
            <button
              type="button"
              aria-label={`Priority: ${PRIORITY_SHORT[priority]}`}
              onClick={() => setPrioMenu((v) => !v)}
              className="flex h-7 items-center rounded-control border border-line bg-card px-2 text-[12px] text-text-3 hover:text-text"
            >
              {priority === "none" ? "No priority" : <PriorityChip priority={priority} />}
            </button>
            {prioMenu && (
              <div className="absolute left-0 top-8 z-10">
                <PriorityMenu
                  value={priority}
                  onPick={(p) => {
                    setPrioMenu(false);
                    setPriority(p);
                  }}
                  onClose={() => setPrioMenu(false)}
                />
              </div>
            )}
          </div>
          <select
            aria-label="Assignee"
            className={chip}
            value={assignee}
            onChange={(e) => {
              const a = e.target.value as Assignee;
              setAssignee(a);
              if (a === "me") setNight(false);
            }}
          >
            <option value="me">Me</option>
            <option value="agent">Agent</option>
          </select>
          <select aria-label="Project" className={`${chip} max-w-40`} value={projectId} onChange={(e) => setProjectId(e.target.value ? Number(e.target.value) : "")}>
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <input
            type="date"
            aria-label="Due date"
            className={`${chip} font-mono`}
            value={due}
            onChange={(e) => {
              setDue(e.target.value);
              if (!e.target.value) setDueTime("");
            }}
          />
          <input
            type="time"
            aria-label="Due time"
            disabled={!due}
            className={`${chip} font-mono disabled:opacity-40`}
            value={dueTime}
            onChange={(e) => setDueTime(e.target.value)}
          />
          <select aria-label="Repeat" className={chip} value={repeat} onChange={(e) => setRepeat(e.target.value as Repeat | "")}>
            <option value="">No repeat</option>
            {REPEATS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
          <span className="flex h-7 items-center gap-2 rounded-control border border-line bg-card px-2 text-[12px] text-text-3">
            <Switch
              label="Night task"
              checked={night}
              onChange={(n) => {
                setNight(n);
                if (n) setAssignee("agent");
              }}
            />
            {night ? <Moon /> : "Night"}
          </span>
        </div>
        <div className="mt-4 flex items-center gap-2 border-t border-line pt-3">
          {error && <span className="min-w-0 truncate text-[12px] text-text-2">{error}</span>}
          <Button variant="ghost" className="ml-auto" onClick={close}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => void submit()}>
            Create task
          </Button>
        </div>
      </div>
    </div>
  );
}
