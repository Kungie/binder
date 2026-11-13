import { useEffect, useState } from "react";
import { bytes } from "../lib/format";
import { useStore } from "../lib/store";
import type { CloudInfo } from "../lib/types";

const KIND = { file: "File", dir: "Folder", link: "Link" } as const;
const when = (unix: number) => (unix ? new Date(unix * 1000).toLocaleString("en-GB", { dateStyle: "medium", timeStyle: "short" }) : "Unknown");

/** Get info: a side sheet with name, kind, size, dates and the Mac path. A folder's size is counted on the server. */
export function FileInfo({ rel, onClose }: { rel: string; onClose(): void }) {
  const { api } = useStore();
  const [info, setInfo] = useState<CloudInfo | null>(null);
  const [size, setSize] = useState<[number, number] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setInfo(null);
    setSize(null);
    api.cloudInfo(rel).then(
      (i) => {
        if (!live) return;
        setInfo(i);
        if (i.entry.kind === "dir") void api.cloudSize(rel).then((s) => live && setSize(s), () => {});
      },
      (e) => live && setError(e instanceof Error ? e.message : String(e)),
    );
    return () => {
      live = false;
    };
  }, [api, rel]);
  const rows: [string, string][] = info
    ? [
        ["Kind", KIND[info.entry.kind]],
        ["Size", info.entry.kind === "dir" ? (size ? `${bytes(size[0])}, ${size[1].toLocaleString("en-US")} ${size[1] === 1 ? "item" : "items"}` : "Counting…") : bytes(info.entry.size)],
        ["Modified", when(info.entry.modified)],
        ["Created", when(info.created)],
      ]
    : [];
  return (
    <aside
      role="dialog"
      aria-label="Info"
      onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), onClose())}
      className="fade-in absolute bottom-0 right-0 top-0 z-30 flex w-[320px] flex-col border-l border-line-strong bg-panel"
    >
      <header className="flex h-12 items-center gap-2 px-4">
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{info?.entry.name ?? ""}</span>
        <button type="button" onClick={onClose} className="text-[12px] text-text-3 hover:text-text">
          Close
        </button>
      </header>
      {error && <p className="px-4 text-[12px] text-urgent">{error}</p>}
      <dl className="flex flex-col border-t border-line text-[12px]">
        {rows.map(([k, v]) => (
          <div key={k} className="grid grid-cols-[84px_minmax(0,1fr)] gap-3 border-b border-line px-4 py-2">
            <dt className="text-text-3">{k}</dt>
            <dd className={k === "Size" ? "font-mono text-[11px] text-text-2" : "text-text-2"}>{v}</dd>
          </div>
        ))}
        {info && (
          <div className="flex flex-col gap-1 px-4 py-2">
            <dt className="text-text-3">Where</dt>
            <dd className="break-all font-mono text-[11px] text-text-2">{info.path}</dd>
          </div>
        )}
      </dl>
    </aside>
  );
}
