import { useCallback, useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { Access, AgentRun as Run, Task } from "../lib/types";
import { usePoll } from "../lib/usePoll";
import { clock } from "../lib/useServer";
import { Button, Field, Notice, fieldClass } from "./ui";

const FULL_WARNING = "Runs without asking. Use only for tasks you trust.";

export function runState(r: Run): string {
  if (r.ended === null) return `Running since ${clock(r.started)}`;
  if (r.exit_code === 0) return "Finished";
  if (r.exit_code === 143) return "Stopped";
  if (r.exit_code === null) return "Lost. The process ended without an exit code.";
  return `Failed with exit code ${r.exit_code}`;
}

/** Polls a live run every 10 seconds. Mounted only while the run is live. */
function Live({ run, onUpdate }: { run: string; onUpdate(r: Run): void }) {
  const { api } = useStore();
  usePoll(() => api.pollRun(run).then(onUpdate, () => {}), 10_000);
  return null;
}

/** Run controls for a task assigned to the agent: where, how much access, and the latest run. */
export function AgentRun({ task }: { task: Task }) {
  const { api, projects, reload } = useStore();
  const project = projects.find((p) => p.id === task.project_id);
  const [dir, setDir] = useState(project?.agent_dir ?? "~");
  const [access, setAccess] = useState<Access>("edits_only");
  const [run, setRun] = useState<Run | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    void api.runsFor(task.id).then((r) => live && setRun(r[0] ?? null));
    return () => {
      live = false;
    };
  }, [api, task.id]);

  const update = useCallback(
    (r: Run) => {
      setRun(r);
      if (r.ended !== null) void reload();
    },
    [reload],
  );

  const act = async (go: () => Promise<Run>) => {
    setBusy(true);
    setError(null);
    try {
      update(await go());
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const live = run !== null && run.ended === null;

  return (
    <section aria-label="Agent run" className="flex flex-col gap-2.5 rounded-control border border-line p-3">
      <Field label="Folder">
        <input className={`${fieldClass} font-mono`} value={dir} onChange={(e) => setDir(e.target.value)} disabled={live} />
      </Field>
      <div role="radiogroup" aria-label="Access" className="grid grid-cols-[88px_minmax(0,1fr)] items-center gap-2 text-[12px]">
        <span className="text-text-3">Access</span>
        <div className="flex gap-3">
          {(
            [
              ["edits_only", "Edits only"],
              ["full", "Full access"],
            ] as [Access, string][]
          ).map(([value, label]) => (
            <label key={value} className="flex items-center gap-1.5 text-text-2">
              <input type="radio" name={`access-${task.id}`} value={value} checked={access === value} onChange={() => setAccess(value)} disabled={live} className="accent-text" />
              {label}
            </label>
          ))}
        </div>
      </div>
      {access === "full" && <Notice>{FULL_WARNING}</Notice>}
      <div className="flex items-center gap-2 text-[12px]">
        {run && <span className="text-text-2">{runState(run)}</span>}
        {live ? (
          <Button className="ml-auto" disabled={busy} onClick={() => void act(() => api.stopRun(run.run))}>
            Stop
          </Button>
        ) : (
          <Button variant="primary" className="ml-auto" disabled={busy || !dir.trim()} onClick={() => void act(() => api.startRun(task.id, dir.trim(), access))}>
            Run
          </Button>
        )}
      </div>
      {error && <Notice>{error}</Notice>}
      {run?.tail && (
        <pre aria-label="Log" className="max-h-48 overflow-auto whitespace-pre-wrap rounded-control bg-card p-2 font-mono text-[11px] leading-relaxed text-text-2">
          {run.tail}
        </pre>
      )}
      {live && <Live run={run.run} onUpdate={update} />}
    </section>
  );
}
