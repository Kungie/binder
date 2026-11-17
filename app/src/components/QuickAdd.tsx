import { useEffect, useRef, useState } from "react";
import type { Api } from "../lib/api";
import { parseQuick } from "../lib/quickadd";
import { repeatInfo } from "../lib/repeat";
import { localDay } from "../lib/tasks";
import type { Project } from "../lib/types";
import { RepeatIcon } from "./RepeatIcon";

const PRIORITY_WORD = { none: "", low: "Low", medium: "Medium", high: "High", urgent: "Urgent" } as const;

/**
 * The quick add window (Control-Option-N anywhere, or New task in the menu bar): one line, read as it's typed,
 * Enter makes the task, Escape closes.
 */
export function QuickAdd({ api }: { api: Api }) {
  const [text, setText] = useState("");
  const [projects, setProjects] = useState<Project[]>([]);
  const [error, setError] = useState<string | null>(null);
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const load = () => void api.listProjects().then(setProjects, () => {});
    load();
    return api.onQuickOpen(() => {
      setText("");
      setError(null);
      load();
      field.current?.focus();
    });
  }, [api]);
  const read = parseQuick(text, projects, localDay());
  const project = projects.find((p) => p.id === read.project_id);
  const submit = async () => {
    if (!text.trim()) return;
    try {
      await api.createTask(read);
      setText("");
      setError(null);
      await api.quickAdded();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  const chips = [
    read.due && `${read.due.slice(8)}.${read.due.slice(5, 7)}${read.due_time ? ` ${read.due_time}` : ""}`,
    read.priority && PRIORITY_WORD[read.priority],
  ].filter(Boolean) as string[];
  return (
    <div className="flex h-screen items-center gap-2 bg-panel px-4" data-tauri-drag-region>
      <input
        ref={field}
        autoFocus
        aria-label="New task"
        placeholder="New task: Pay rent every month #home !high"
        value={text}
        autoCorrect="off"
        spellCheck={false}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void submit();
          if (e.key === "Escape") void api.quickClose();
        }}
        className="h-9 min-w-0 flex-1 bg-transparent text-[15px] text-text outline-none placeholder:text-text-3"
      />
      {error ? (
        <span className="shrink-0 text-[12px] text-bad">{error}</span>
      ) : (
        <span className="flex shrink-0 items-center gap-1 text-[11px] text-text-2">
          {chips.map((c) => (
            <span key={c} className="rounded-full bg-card px-2 py-0.5 font-mono">
              {c}
            </span>
          ))}
          {read.repeat && (
            <span className="flex items-center gap-1 rounded-full bg-card px-2 py-0.5">
              <RepeatIcon />
              {repeatInfo(read.repeat).short}
            </span>
          )}
          {project && (
            <span className="flex items-center gap-1 rounded-full bg-card px-2 py-0.5">
              <span className="size-1.5 rounded-full" style={{ backgroundColor: project.color }} aria-hidden="true" />
              {project.name}
            </span>
          )}
        </span>
      )}
    </div>
  );
}
