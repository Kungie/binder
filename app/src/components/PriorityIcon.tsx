import { PRIORITY_COLOR } from "../lib/colors";
import { type Priority, PRIORITY_LABEL } from "../lib/types";
import { Chip } from "./Chip";

const LEVEL: Record<Priority, number> = { none: 0, low: 1, medium: 2, high: 3, urgent: 4 };

export function PriorityIcon({ priority, size = 14 }: { priority: Priority; size?: number }) {
  const label = PRIORITY_LABEL[priority];
  if (priority === "urgent") {
    return (
      <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={label} className="shrink-0">
        <rect x="1.5" y="1.5" width="11" height="11" rx="2.5" fill={PRIORITY_COLOR.urgent} />
        <path d="M7 4v3.6" stroke="var(--color-bg)" strokeWidth="1.6" strokeLinecap="round" />
        <circle cx="7" cy="9.9" r="0.9" fill="var(--color-bg)" />
      </svg>
    );
  }
  if (priority === "none") {
    return (
      <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={label} className="shrink-0">
        {[3, 7, 11].map((x) => (
          <circle key={x} cx={x} cy="7" r="1" fill="var(--color-text-3)" />
        ))}
      </svg>
    );
  }
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={label} className="shrink-0">
      {[0, 1, 2].map((i) => (
        <rect
          key={i}
          data-bar={i < LEVEL[priority] ? "on" : "off"}
          x={2 + i * 4}
          y={9 - i * 3}
          width="2.4"
          height={3 + i * 3}
          rx="0.6"
          fill={i < LEVEL[priority] ? PRIORITY_COLOR[priority] : "var(--color-line-strong)"}
        />
      ))}
    </svg>
  );
}

/** Short names for chips and menus. */
export const PRIORITY_SHORT: Record<Priority, string> = { none: "No priority", low: "Low", medium: "Medium", high: "High", urgent: "Urgent" };

/** The priority as a chip, with its icon in colour and the text in the same tone. Nothing for no priority. */
export function PriorityChip({ priority }: { priority: Priority }) {
  if (priority === "none") return null;
  return (
    <Chip icon={<PriorityIcon priority={priority} size={12} />}>
      <span style={{ color: PRIORITY_COLOR[priority] }}>{PRIORITY_SHORT[priority]}</span>
    </Chip>
  );
}
