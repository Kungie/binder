import { useState } from "react";
import { useStore } from "../lib/store";

/** After a task in a project is done: one line for the project's done log, or Esc to skip. */
export function DonePrompt({ projectId, projectName, initial, onClose }: { projectId: number; projectName: string; initial: string; onClose(): void }) {
  const { logDone } = useStore();
  const [text, setText] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const label = `Log to ${projectName}`;

  const submit = async () => {
    if (!text.trim()) return onClose();
    setBusy(true);
    try {
      await logDone(projectId, text);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fade-in flex items-center gap-3 border-t border-line bg-panel px-3 py-2 text-[12px]">
      <label htmlFor="done-prompt" className="shrink-0 text-text-2">
        {label}
      </label>
      <input
        id="done-prompt"
        aria-label={label}
        autoFocus
        value={text}
        disabled={busy}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") void submit();
          if (e.key === "Escape") onClose();
        }}
        className="h-7 min-w-0 flex-1 rounded-control border border-line-strong bg-bg px-2 focus:border-text-3 focus:outline-none"
      />
      {error ? <span className="shrink-0 text-text">{error.startsWith("Vault not connected") ? "Vault not connected" : error}</span> : <span className="shrink-0 font-mono text-[11px] text-text-3">enter · esc</span>}
    </div>
  );
}
