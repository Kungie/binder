import { type Status, STATUS_LABEL } from "../lib/types";

const COLOUR: Record<Status, string> = {
  backlog: "text-st-backlog",
  todo: "text-st-todo",
  in_progress: "text-st-progress",
  done: "text-st-done",
  canceled: "text-st-canceled",
};

/** Linear-style status circles: the shape carries the state, the colour repeats it. */
export function StatusIcon({ status, size = 14 }: { status: Status; size?: number }) {
  const stroke = "currentColor";
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" role="img" aria-label={STATUS_LABEL[status]} className={`shrink-0 ${COLOUR[status]}`}>
      {status === "backlog" && <circle cx="7" cy="7" r="5.5" fill="none" stroke={stroke} strokeWidth="1.5" strokeDasharray="2 2" />}
      {status === "todo" && <circle cx="7" cy="7" r="5.5" fill="none" stroke={stroke} strokeWidth="1.5" />}
      {status === "in_progress" && (
        <>
          <circle cx="7" cy="7" r="5.5" fill="none" stroke={stroke} strokeWidth="1.5" />
          <path data-fill="half" d="M7 3.5 A3.5 3.5 0 0 1 7 10.5 Z" fill={stroke} />
        </>
      )}
      {status === "done" && (
        <>
          <circle cx="7" cy="7" r="6.25" fill={stroke} />
          <path d="M4.4 7.2 6.2 9 9.7 5.3" fill="none" stroke="var(--color-panel)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </>
      )}
      {status === "canceled" && (
        <>
          <circle cx="7" cy="7" r="5.5" fill="none" stroke={stroke} strokeWidth="1.5" />
          <path d="M4.5 9.5 9.5 4.5" stroke={stroke} strokeWidth="1.5" strokeLinecap="round" />
        </>
      )}
    </svg>
  );
}
