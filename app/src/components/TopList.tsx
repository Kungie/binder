import { useEffect, useState } from "react";
import { bytes } from "../lib/format";
import { useStore } from "../lib/store";
import type { TopFolder, TopProcess } from "../lib/types";
import { Panel } from "./Panel";

export type TopKind = "cpu" | "memory" | "disk";

const TITLE: Record<TopKind, string> = { cpu: "Processes by CPU", memory: "Processes by memory", disk: "Largest folders" };

type Rows = { kind: "processes"; rows: TopProcess[] } | { kind: "folders"; rows: TopFolder[] };

/** What uses the server, largest first, read when its box is pressed. */
export function TopList({ kind }: { kind: TopKind }) {
  const { api } = useStore();
  const [rows, setRows] = useState<Rows | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setRows(null);
    setError(null);
    const read: Promise<Rows> =
      kind === "disk" ? api.serverFolders().then((r) => ({ kind: "folders", rows: r })) : api.serverProcesses(kind === "memory").then((r) => ({ kind: "processes", rows: r }));
    read.then(
      (r) => live && setRows(r),
      (e: unknown) => live && setError(e instanceof Error ? e.message : String(e)),
    );
    return () => {
      live = false;
    };
  }, [api, kind]);

  const cell = "py-1.5 font-normal";
  return (
    <Panel title={TITLE[kind]} label={TITLE[kind]}>
      {!rows && !error && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">{kind === "disk" ? "Counting…" : "Reading…"}</p>}
      {error && <p className="border-t border-line px-3 py-3 text-[12px] text-bad">{error}</p>}
      {rows && (
        <table className="w-full table-fixed border-t border-line text-[12px]">
          <thead>
            <tr className="text-left text-text-3">
              <th className={`${cell} pl-3`}>{rows.kind === "folders" ? "Folder" : "Process"}</th>
              {rows.kind === "processes" && kind === "cpu" && <th className={`${cell} w-24 text-right`}>CPU</th>}
              {rows.kind === "processes" && kind === "memory" && <th className={`${cell} w-24 text-right`}>Memory</th>}
              <th className={`${cell} w-24 pr-3 text-right`}>{rows.kind === "folders" ? "Size" : "Share"}</th>
            </tr>
          </thead>
          <tbody>
            {rows.kind === "processes" &&
              rows.rows.map((p, i) => (
                <tr key={`${p.name}-${i}`} className="border-t border-line">
                  <td className="truncate py-1.5 pl-3 font-mono">{p.name}</td>
                  <td className="py-1.5 text-right font-mono text-text-2">{kind === "cpu" ? `${p.cpu.toFixed(1)}%` : `${p.rss_mb} MB`}</td>
                  <td className="py-1.5 pr-3 text-right font-mono text-text-3">{`${p.mem.toFixed(1)}% mem`}</td>
                </tr>
              ))}
            {rows.kind === "folders" &&
              rows.rows.map((f) => (
                <tr key={f.path} className="border-t border-line">
                  <td className="truncate py-1.5 pl-3 font-mono">{f.path}</td>
                  <td className="py-1.5 pr-3 text-right font-mono text-text-2">{bytes(f.bytes)}</td>
                </tr>
              ))}
          </tbody>
        </table>
      )}
    </Panel>
  );
}
