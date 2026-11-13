import { useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { Assignee } from "../lib/types";
import { Button, fieldClass } from "./ui";

/** The new-task dialog: a title, then project and assignee. Enter creates, Esc cancels. */
export function NewTask() {
  const { projects, view, createTask, setCreatingTask, screen, selectedProject } = useStore();
  const preset = view.kind === "project" ? view.id : screen === "projects" ? selectedProject : null;
  const [title, setTitle] = useState("");
  const [projectId, setProjectId] = useState<number | "">(preset ?? "");
  const [assignee, setAssignee] = useState<Assignee>(view.kind === "agent" ? "agent" : "me");
  const [error, setError] = useState<string | null>(null);
  const close = () => setCreatingTask(false);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const submit = async () => {
    if (!title.trim()) return setError("Give it a title.");
    try {
      await createTask({ title, project_id: projectId === "" ? undefined : projectId, assignee });
      close();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[18vh]" onMouseDown={close}>
      <div
        role="dialog"
        aria-label="New task"
        className="fade-in w-[520px] rounded-panel border border-line-strong bg-panel p-4 shadow-menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter" && !e.shiftKey) {
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
        <div className="mt-4 flex items-center gap-2">
          <select aria-label="Project" className={`${fieldClass} w-40`} value={projectId} onChange={(e) => setProjectId(e.target.value ? Number(e.target.value) : "")}>
            <option value="">No project</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <select aria-label="Assignee" className={`${fieldClass} w-28`} value={assignee} onChange={(e) => setAssignee(e.target.value as Assignee)}>
            <option value="me">Me</option>
            <option value="agent">Agent</option>
          </select>
          {error && <span className="text-[12px] text-text-2">{error}</span>}
          <Button variant="primary" className="ml-auto" onClick={() => void submit()}>
            Create task
          </Button>
        </div>
      </div>
    </div>
  );
}
