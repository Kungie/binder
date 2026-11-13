import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { PRIORITIES, type Priority } from "../lib/types";
import { PRIORITY_SHORT, PriorityIcon } from "./PriorityIcon";

/** Most urgent first, as Linear lists them. */
const ORDER: Priority[] = [...PRIORITIES].reverse();

/** A small priority picker. It takes the keys itself: arrows or j/k move, Enter or 1-5 pick, Esc closes. */
export function PriorityMenu({ value, onPick, onClose }: { value: Priority; onPick(p: Priority): void; onClose(): void }) {
  const [active, setActive] = useState(Math.max(0, ORDER.indexOf(value)));
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => ref.current?.focus(), []);

  const onKeyDown = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "ArrowDown" || e.key === "j") setActive((a) => Math.min(ORDER.length - 1, a + 1));
    else if (e.key === "ArrowUp" || e.key === "k") setActive((a) => Math.max(0, a - 1));
    else if (e.key === "Enter") onPick(ORDER[active]);
    else if (e.key === "Escape") onClose();
    else if (/^[1-5]$/.test(e.key)) onPick(ORDER[Number(e.key) - 1]);
    else return;
    e.preventDefault();
  };

  return (
    <div
      ref={ref}
      role="listbox"
      aria-label="Set priority"
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onBlur={(e) => !e.currentTarget.contains(e.relatedTarget) && onClose()}
      className="fade-in z-30 w-44 rounded-control border border-line-strong bg-raised p-1 shadow-menu outline-none"
    >
      {ORDER.map((p, i) => (
        <div
          key={p}
          role="option"
          aria-selected={p === value}
          onMouseDown={(e) => {
            e.preventDefault();
            onPick(p);
          }}
          onMouseEnter={() => setActive(i)}
          className={`flex h-7 cursor-default items-center gap-2 rounded-[4px] px-2 text-[12px] ${i === active ? "bg-line-strong text-text" : "text-text-2"}`}
        >
          <PriorityIcon priority={p} />
          {PRIORITY_SHORT[p]}
          <span className="ml-auto font-mono text-[11px] text-text-3">{i + 1}</span>
        </div>
      ))}
    </div>
  );
}
