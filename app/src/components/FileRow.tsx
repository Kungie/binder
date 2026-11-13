import { type DragEvent, useEffect, useRef, useState } from "react";
import { bytes } from "../lib/format";
import { ago } from "../lib/time";
import type { CloudEntry } from "../lib/types";

const Folder = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" className="shrink-0 text-[#8B7CF6]">
    <path d="M1.5 3.5h4l1.2 1.4h5.8v6.6h-11z" fill="currentColor" fillOpacity="0.25" stroke="currentColor" strokeWidth="1.1" strokeLinejoin="round" />
  </svg>
);

const File = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.1" aria-hidden="true" className="shrink-0 text-text-3">
    <path d="M3.5 1.5h4.5l2.5 2.5v8.5h-7z M8 1.5V4h2.5" strokeLinejoin="round" />
  </svg>
);

const Link = () => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.1" aria-hidden="true" className="shrink-0 text-text-3">
    <path d="M6 8a2.5 2.5 0 0 0 3.5 0l2-2A2.5 2.5 0 0 0 8 2.5l-.7.7M8 6a2.5 2.5 0 0 0-3.5 0l-2 2A2.5 2.5 0 0 0 6 11.5l.7-.7" strokeLinecap="round" />
  </svg>
);

export function KindIcon({ entry }: { entry: CloudEntry }) {
  return entry.kind === "dir" ? <Folder /> : entry.kind === "link" ? <Link /> : <File />;
}

/** An inline name field: Enter saves, Esc or leaving without a change cancels. */
export function NameInput({ label, initial, onDone }: { label: string; initial: string; onDone(name: string | null): void }) {
  const [name, setName] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    const el = ref.current;
    el?.focus();
    // Select the name without its extension, as Finder does.
    const dot = initial.lastIndexOf(".");
    el?.setSelectionRange(0, dot > 0 ? dot : initial.length);
  }, [initial]);
  const finish = (v: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(v);
  };
  return (
    <input
      ref={ref}
      aria-label={label}
      value={name}
      onChange={(e) => setName(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") finish(name.trim() && name !== initial ? name : null);
        if (e.key === "Escape") finish(null);
      }}
      onBlur={() => finish(name.trim() && name !== initial ? name : null)}
      className="h-6 w-full min-w-0 rounded-control border border-text-3 bg-card px-1.5 text-[12px] text-text focus:outline-none"
    />
  );
}

export interface FileRowProps {
  entry: CloudEntry;
  selected: boolean;
  renaming: boolean;
  dropTarget: boolean;
  onSelect(): void;
  onOpen(): void;
  onRename(name: string | null): void;
  onDragStart(e: DragEvent): void;
  onDragOver(e: DragEvent): void;
  onDragLeave(): void;
  onDrop(e: DragEvent): void;
}

export function FileRow({ entry, selected, renaming, dropTarget, onSelect, onOpen, onRename, onDragStart, onDragOver, onDragLeave, onDrop }: FileRowProps) {
  const dir = entry.kind === "dir";
  return (
    <tr
      data-name={entry.name}
      aria-selected={selected}
      draggable={!renaming}
      onClick={onSelect}
      onDoubleClick={onOpen}
      onDragStart={onDragStart}
      onDragOver={dir ? onDragOver : undefined}
      onDragLeave={dir ? onDragLeave : undefined}
      onDrop={dir ? onDrop : undefined}
      className={`h-8 cursor-default border-b border-line text-[12px] last:border-b-0 ${selected ? "bg-raised" : "hover:bg-raised/50"} ${dropTarget ? "outline outline-1 -outline-offset-1 outline-text-3" : ""}`}
    >
      <td className="px-3">
        <div className="flex min-w-0 items-center gap-2">
          <KindIcon entry={entry} />
          {renaming ? <NameInput label={`Rename ${entry.name}`} initial={entry.name} onDone={onRename} /> : <span className="truncate text-text">{entry.name}</span>}
        </div>
      </td>
      <td className="w-24 px-3 text-right font-mono text-[11px] text-text-3">{dir ? "" : bytes(entry.size)}</td>
      <td className="w-32 px-3 text-text-3">{entry.modified ? ago(entry.modified) : ""}</td>
    </tr>
  );
}
