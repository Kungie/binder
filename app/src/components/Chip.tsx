import type { ReactNode } from "react";

/** A pill under a card or beside a title: an icon and a few words. `warn` turns the text orange (overdue). */
export function Chip({ icon, children, tone = "default", title }: { icon?: ReactNode; children: ReactNode; tone?: "default" | "warn"; title?: string }) {
  return (
    <span
      data-chip
      title={title}
      className={`inline-flex h-6 max-w-full shrink-0 items-center gap-1.5 rounded-full border border-line bg-card px-2.5 text-[12px] ${
        tone === "warn" ? "text-urgent" : "text-text-2"
      }`}
    >
      {icon && <span className="flex shrink-0 items-center">{icon}</span>}
      <span className="truncate">{children}</span>
    </span>
  );
}
