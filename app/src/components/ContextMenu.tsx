import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface MenuItem {
  label: string;
  /** Shown on the right in text-3. */
  shortcut?: string;
  submenu?: MenuItem[];
  onPick?(): void;
}

export type MenuEntry = MenuItem | "separator";

const MARGIN = 4;

/** Binder's own right-click menu: a raised panel kept inside the window. Arrows move, Enter picks, Esc closes,
 * ArrowRight opens a submenu and ArrowLeft closes it. */
export function ContextMenu({
  x,
  y,
  items,
  onClose,
  label = "Menu",
  nested = false,
  onBack,
}: {
  x: number;
  y: number;
  items: MenuEntry[];
  onClose(): void;
  label?: string;
  nested?: boolean;
  /** For a submenu: ArrowLeft goes back to its parent. */
  onBack?(): void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });
  const picks = items.map((it, i) => (it === "separator" ? -1 : i)).filter((i) => i >= 0);
  // A submenu opened from the keyboard starts on its first item.
  const [active, setActive] = useState(nested ? (picks[0] ?? -1) : -1);
  const [open, setOpen] = useState<number | null>(null);

  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    setPos({ x: Math.max(MARGIN, Math.min(x, window.innerWidth - r.width - MARGIN)), y: Math.max(MARGIN, Math.min(y, window.innerHeight - r.height - MARGIN)) });
  }, [x, y]);

  useEffect(() => {
    if (open === null) ref.current?.focus();
  }, [open]);

  useEffect(() => {
    if (nested) return;
    const away = (e: MouseEvent) => {
      if (!(e.target instanceof Node) || !document.querySelector("[data-context-menu]")?.contains(e.target)) onClose();
    };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [nested, onClose]);

  const pick = (item: MenuItem) => {
    if (item.submenu) return;
    onClose();
    item.onPick?.();
  };
  const step = (by: number) => {
    const at = picks.indexOf(active);
    setActive(picks[at < 0 ? (by > 0 ? 0 : picks.length - 1) : (at + by + picks.length) % picks.length]);
  };

  const body = (
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      style={nested ? undefined : { left: pos.x, top: pos.y }}
      onKeyDown={(e) => {
        if (open !== null) return;
        e.stopPropagation();
        const item = items[active];
        if (e.key === "ArrowDown") return e.preventDefault(), step(1);
        if (e.key === "ArrowUp") return e.preventDefault(), step(-1);
        if (e.key === "Escape") return e.preventDefault(), onClose();
        if (nested && e.key === "ArrowLeft") return e.preventDefault(), onBack?.();
        if (item && item !== "separator") {
          if (e.key === "ArrowRight" && item.submenu) return e.preventDefault(), setOpen(active);
          if (e.key === "Enter" || e.key === " ") return e.preventDefault(), item.submenu ? setOpen(active) : pick(item);
        }
      }}
      onContextMenu={(e) => e.preventDefault()}
      className={`${nested ? "absolute left-full top-0 ml-1" : "fixed z-50"} min-w-[200px] rounded-control border border-line-strong bg-raised py-1 shadow-menu focus:outline-none`}
    >
      {items.map((item, i) =>
        item === "separator" ? (
          <div key={`sep-${i}`} role="separator" className="my-1 h-px bg-line" />
        ) : (
          <div key={item.label} className="relative">
            <div
              role="menuitem"
              aria-label={item.label}
              aria-haspopup={item.submenu ? "menu" : undefined}
              data-active={active === i}
              onMouseEnter={() => (setActive(i), setOpen(item.submenu ? i : null))}
              onClick={() => pick(item)}
              className={`flex h-7 cursor-default items-center gap-4 px-3 text-[12px] ${active === i ? "bg-line text-text" : "text-text-2"}`}
            >
              <span className="flex-1 truncate">{item.label}</span>
              {item.shortcut && <span className="font-mono text-[11px] text-text-3">{item.shortcut}</span>}
              {item.submenu && <span className="text-text-3" aria-hidden="true">›</span>}
            </div>
            {open === i && item.submenu && <ContextMenu nested x={0} y={0} label={item.label} items={item.submenu} onClose={onClose} onBack={() => setOpen(null)} />}
          </div>
        ),
      )}
    </div>
  );
  return nested ? body : <div data-context-menu>{body}</div>;
}
