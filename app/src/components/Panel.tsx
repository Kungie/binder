import type { ReactNode } from "react";

export function Panel({
  title,
  action,
  children,
  className = "",
  label,
}: {
  title?: ReactNode;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Names the panel for assistive tech, which makes it a region. */
  label?: string;
}) {
  return (
    <section aria-label={label} className={`min-w-0 rounded-panel border border-line bg-panel ${className}`}>
      {title !== undefined && (
        <header className="flex h-9 items-center gap-2 px-3 text-[12px] text-text-2">
          <span className="font-medium text-text">{title}</span>
          <span className="ml-auto flex items-center gap-2">{action}</span>
        </header>
      )}
      {children}
    </section>
  );
}
