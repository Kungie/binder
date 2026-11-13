import { useCallback, useState } from "react";
import { useStore } from "../lib/store";
import type { Status, Task } from "../lib/types";
import { DonePrompt } from "./DonePrompt";
import { TaskSheet } from "./TaskSheet";
import { TaskTable } from "./TaskTable";

/** A task table wired to the store: status changes, the done prompt and the side sheet. */
export function TaskBoard({ tasks, compact, label, empty }: { tasks: Task[]; compact?: boolean; label?: string; empty?: string }) {
  const store = useStore();
  const [openId, setOpenId] = useState<number | null>(null);
  const [prompt, setPrompt] = useState<{ projectId: number; name: string; text: string } | null>(null);
  const open = store.tasks.find((t) => t.id === openId);

  const onStatus = async (ids: number[], status: Status) => {
    const before = ids.map((id) => store.tasks.find((t) => t.id === id)).filter(Boolean) as Task[];
    await Promise.all(ids.map((id) => store.updateTask(id, { status })));
    const only = before.length === 1 ? before[0] : null;
    if (status === "done" && only?.project_id && only.status !== "done") {
      const project = store.projects.find((p) => p.id === only.project_id);
      if (project) setPrompt({ projectId: project.id, name: project.name, text: only.title });
    }
  };

  const close = useCallback(() => setOpenId(null), []);

  return (
    <>
      <TaskTable
        tasks={tasks}
        projects={store.projects}
        compact={compact}
        label={label}
        empty={empty}
        onOpen={setOpenId}
        onCreate={() => store.setCreatingTask(true)}
        onStatus={(ids, s) => void onStatus(ids, s)}
        onAssign={(id, assignee) => void store.updateTask(id, { assignee })}
      />
      {prompt && <DonePrompt key={prompt.projectId + prompt.text} projectId={prompt.projectId} projectName={prompt.name} initial={prompt.text} onClose={() => setPrompt(null)} />}
      {open && <TaskSheet task={open} onClose={close} />}
    </>
  );
}
