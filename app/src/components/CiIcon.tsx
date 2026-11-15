import type { Ci } from "../lib/types";

const LABEL = { passing: "CI passing", failing: "CI failing", running: "CI running" } as const;
const COLOR = { passing: "text-ok", failing: "text-bad", running: "text-st-progress" } as const;

/** Checks on a branch as a small icon in its meaning colour: a tick, a cross or a dot. */
export function CiIcon({ ci }: { ci: Ci }) {
  return (
    <svg role="img" aria-label={LABEL[ci]} viewBox="0 0 12 12" className={`size-3 shrink-0 ${COLOR[ci]}`} fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
      {ci === "passing" && <path d="M2.5 6.2l2.3 2.3 4.7-5" />}
      {ci === "failing" && <path d="M3 3l6 6M9 3l-6 6" />}
      {ci === "running" && <circle cx="6" cy="6" r="2.2" fill="currentColor" stroke="none" />}
    </svg>
  );
}
