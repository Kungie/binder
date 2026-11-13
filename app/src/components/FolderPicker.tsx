import { useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { CloudEntry } from "../lib/types";
import { Button } from "./ui";

/** Move to…: walk Cloud's folders and pick one. */
export function FolderPicker({ onPick, onClose }: { onPick(destRel: string): void; onClose(): void }) {
  const { api } = useStore();
  const [rel, setRel] = useState("");
  const [dirs, setDirs] = useState<CloudEntry[]>([]);
  useEffect(() => {
    let live = true;
    void api.cloudList(rel, false).then((l) => live && setDirs(l.filter((e) => e.kind === "dir")), () => live && setDirs([]));
    return () => {
      live = false;
    };
  }, [api, rel]);
  const up = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
  return (
    <div className="absolute inset-0 z-40 flex items-center justify-center bg-black/40" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div role="dialog" aria-label="Move to" onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), onClose())} className="flex w-[360px] flex-col rounded-panel border border-line-strong bg-panel shadow-menu">
        <header className="flex h-10 items-center gap-2 px-3 text-[12px]">
          {rel && (
            <button type="button" aria-label="Up" onClick={() => setRel(up)} className="text-text-3 hover:text-text">
              ‹
            </button>
          )}
          <span className="truncate font-mono text-[11px] text-text-2">{rel ? `Cloud/${rel}` : "Cloud"}</span>
        </header>
        <ul className="max-h-[280px] overflow-y-auto border-y border-line">
          {dirs.map((d) => (
            <li key={d.rel}>
              <button type="button" onClick={() => setRel(d.rel)} className="flex h-8 w-full items-center px-3 text-left text-[12px] text-text-2 hover:bg-raised">
                {d.name}
              </button>
            </li>
          ))}
          {dirs.length === 0 && <li className="px-3 py-3 text-[12px] text-text-3">No folders here.</li>}
        </ul>
        <footer className="flex justify-end gap-2 p-3">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => onPick(rel)}>
            Move here
          </Button>
        </footer>
      </div>
    </div>
  );
}
