import { useEffect, useRef, useState } from "react";
import { bytes } from "../lib/format";
import { useStore } from "../lib/store";
import type { Transfer } from "../lib/types";
import { Progress } from "./Progress";

interface Row {
  t: Transfer;
  started: number;
}

const FADE_MS = 4000;

function eta(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  if (seconds < 60) return `${Math.ceil(seconds)} s left`;
  if (seconds < 3600) return `${Math.ceil(seconds / 60)} min left`;
  return `${(seconds / 3600).toFixed(1)} h left`;
}

/** Copies in flight, bottom right: progress, speed and time left, with cancel. Done rows fade; failures stay. */
export function Transfers() {
  const { api } = useStore();
  const [rows, setRows] = useState<Map<number, Row>>(new Map());
  const timers = useRef<number[]>([]);

  useEffect(() => {
    const off = api.onTransfer((t) => {
      setRows((m) => {
        const next = new Map(m);
        next.set(t.id, { t, started: m.get(t.id)?.started ?? Date.now() });
        return next;
      });
      if (t.state === "done" || t.state === "cancelled") {
        timers.current.push(
          window.setTimeout(
            () =>
              setRows((m) => {
                const next = new Map(m);
                if (next.get(t.id)?.t.state !== "running") next.delete(t.id);
                return next;
              }),
            FADE_MS,
          ),
        );
      }
    });
    const pending = timers.current;
    return () => {
      off();
      pending.forEach(clearTimeout);
    };
  }, [api]);

  if (rows.size === 0) return null;
  const dismiss = (id: number) =>
    setRows((m) => {
      const next = new Map(m);
      next.delete(id);
      return next;
    });

  return (
    <section aria-label="Transfers" className="fade-in absolute bottom-4 right-4 z-20 flex w-[340px] flex-col rounded-panel border border-line-strong bg-panel shadow-menu">
      <header className="flex h-8 items-center px-3 text-[12px] font-medium">Transfers</header>
      <ul className="flex max-h-[240px] flex-col overflow-y-auto border-t border-line">
        {[...rows.values()].map(({ t, started }) => {
          const secs = (Date.now() - started) / 1000;
          const speed = secs > 0.5 ? t.done / secs : 0;
          const pct = t.total ? (t.done / t.total) * 100 : t.state === "done" ? 100 : 0;
          return (
            <li key={t.id} className={`flex flex-col gap-1.5 border-b border-line px-3 py-2 text-[12px] last:border-b-0 ${t.state === "done" ? "opacity-60" : ""}`}>
              <div className="flex min-w-0 items-center gap-2">
                <span className="min-w-0 flex-1 truncate text-text">{t.name}</span>
                {t.state === "running" && (
                  <button type="button" aria-label={`Cancel ${t.name}`} onClick={() => void api.cloudCancel(t.id)} className="text-[11px] text-text-3 hover:text-text">
                    Cancel
                  </button>
                )}
                {t.state === "failed" && (
                  <button type="button" aria-label={`Dismiss ${t.name}`} onClick={() => dismiss(t.id)} className="text-[11px] text-text-3 hover:text-text">
                    Dismiss
                  </button>
                )}
              </div>
              <Progress value={pct} label={t.name} />
              <div className="flex gap-2 font-mono text-[11px] text-text-3">
                {t.state === "running" && (
                  <>
                    <span>{`${bytes(t.done)} of ${bytes(t.total)}`}</span>
                    {speed > 0 && <span>{`${bytes(speed)}/s`}</span>}
                    {speed > 0 && <span className="ml-auto">{eta((t.total - t.done) / speed)}</span>}
                  </>
                )}
                {t.state === "done" && <span>Done</span>}
                {t.state === "cancelled" && <span>Cancelled</span>}
                {t.state === "failed" && <span className="font-sans text-bad">{t.error ?? "Failed."}</span>}
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
