import { STATUSES, STATUS_LABEL, type Status } from "../lib/types";
import { StatusIcon } from "./StatusIcon";

/** The status picker the table opens with `s`. The table owns the keys; this only draws. */
export function StatusMenu({ active, onPick }: { active: number; onPick(s: Status): void }) {
  return (
    <div role="listbox" aria-label="Set status" className="fade-in absolute right-3 z-20 w-48 rounded-panel bg-raised p-1 shadow-menu">
      {STATUSES.map((s, i) => (
        <div
          key={s}
          role="option"
          aria-selected={i === active}
          onMouseDown={(e) => {
            e.preventDefault();
            onPick(s);
          }}
          className={`flex h-7 items-center gap-2 rounded-[4px] px-2 text-[12px] ${i === active ? "bg-line-strong text-text" : "text-text-2"}`}
        >
          <StatusIcon status={s} />
          {STATUS_LABEL[s]}
          <span className="ml-auto font-mono text-[11px] text-text-3">{i + 1}</span>
        </div>
      ))}
    </div>
  );
}
