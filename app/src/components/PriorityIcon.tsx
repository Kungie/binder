import { type Priority, PRIORITY_LABEL } from "../lib/types";

const LEVEL: Record<Priority, number> = { none: 0, low: 1, medium: 2, high: 3, urgent: 4 };

export function PriorityIcon({ priority, size = 14 }: { priority: Priority; size?: number }) {
  const label = PRIORITY_LABEL[priority];
  if (priority === "urgent") {
    return (
      <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={label} className="shrink-0">
        <rect x="1.5" y="1.5" width="11" height="11" rx="2.5" fill="var(--color-text)" />
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
          fill={i < LEVEL[priority] ? "var(--color-text)" : "var(--color-line-strong)"}
        />
      ))}
    </svg>
  );
}
