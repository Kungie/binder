import { type KeyboardEvent, type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { OverviewLayout } from "../lib/types";
import { DEFAULT_LAYOUT, FILLS, HEIGHT_MIN, SPLIT_MAX, SPLIT_MIN, WIDGETS, clampHeight, clampSplit, move, step } from "./OverviewWidgets";
import { Button } from "./ui";

type Side = "left" | "right";
type Over = { side: Side; before: string | null };

/** Add widget and Reset, shown while editing. */
export function EditBar({ layout, save }: { layout: OverviewLayout; save(l: OverviewLayout): void }) {
  const [menu, setMenu] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const shown = new Set([...layout.left, ...layout.right]);
  const hidden = Object.keys(WIDGETS).filter((id) => !shown.has(id));
  useEffect(() => {
    if (!menu) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setMenu(false);
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [menu]);
  return (
    <>
      <div ref={box} className="relative">
        <Button variant="ghost" disabled={hidden.length === 0} onClick={() => setMenu((m) => !m)}>
          Add widget
        </Button>
        {menu && (
          <div
            role="menu"
            aria-label="Add widget"
            className="pop-in absolute right-0 top-8 z-20 flex min-w-40 flex-col rounded-control bg-raised p-1 shadow-menu"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.stopPropagation();
                setMenu(false);
              }
            }}
          >
            {hidden.map((id) => (
              <button
                key={id}
                type="button"
                role="menuitem"
                className="h-7 rounded-[6px] px-2 text-left text-[12px] text-text-2 hover:bg-card hover:text-text"
                onClick={() => {
                  setMenu(false);
                  // A widget goes back to the column it lives in by default; a new one to the shorter column.
                  const side: Side = DEFAULT_LAYOUT.left.includes(id) ? "left" : DEFAULT_LAYOUT.right.includes(id) ? "right" : layout.left.length <= layout.right.length ? "left" : "right";
                  save({ ...layout, [side]: [...layout[side], id] });
                }}
              >
                {WIDGETS[id]}
              </button>
            ))}
          </div>
        )}
      </div>
      <Button variant="ghost" onClick={() => save({ ...DEFAULT_LAYOUT, heights: {} })}>
        Reset
      </Button>
    </>
  );
}

const ARROWS: Record<string, "up" | "down" | "left" | "right"> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right" };

/**
 * Overview's two columns. While editing, a widget's handle drags it (or arrow keys step it), its bottom edge sets
 * its height (double-click fits it again), and the gutter sets how wide the columns are. Pointer events throughout,
 * since Tauri swallows HTML5 drags.
 */
export function OverviewGrid({ layout, editing, save, render }: { layout: OverviewLayout; editing: boolean; save(l: OverviewLayout): void; render(id: string): ReactNode }) {
  const grid = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<{ id: string; x: number; y: number } | null>(null);
  const [over, setOver] = useState<Over | null>(null);
  // While a pointer resizes, the size shows live and is saved on release.
  const [live, setLive] = useState<{ id: string; h: number } | { split: number } | null>(null);
  const latest = useRef({ layout, over, live, save });
  latest.current = { layout, over, live, save };
  const refocus = useRef<string | null>(null);
  const split = live && "split" in live ? live.split : (layout.split ?? 50);
  const heights = layout.heights ?? {};
  const heightOf = (id: string) => (live && "id" in live && live.id === id ? live.h : heights[id]);

  // A widget stepped to the other column is a new element: give its handle the focus back.
  useLayoutEffect(() => {
    if (!refocus.current) return;
    grid.current?.querySelector<HTMLElement>(`[aria-label="Move ${WIDGETS[refocus.current]}"]`)?.focus();
    refocus.current = null;
  }, [layout]);

  useEffect(() => {
    if (!dragging) return;
    const target = (x: number, y: number): Over | null => {
      const el = document.elementFromPoint(x, y);
      const w = el?.closest("[data-widget]");
      const col = el?.closest("[data-column]");
      if (!col) return null;
      const side = col.getAttribute("data-column") as Side;
      const id = w?.getAttribute("data-widget") ?? null;
      return { side, before: id === dragging.id ? null : id };
    };
    const onMove = (e: PointerEvent | MouseEvent) => {
      setOver(target(e.clientX, e.clientY));
      setDragging((d) => d && { ...d, x: e.clientX, y: e.clientY });
    };
    const onUp = () => {
      const { layout: l, over: o, save: s } = latest.current;
      if (o) s(move(l, dragging.id, o.side, o.before));
      setDragging(null);
      setOver(null);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
    return () => {
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
  }, [dragging?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Follows the pointer until release, then saves what `live` holds. */
  const track = (onMove: (e: PointerEvent) => void) => {
    const up = () => {
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", up);
      const { layout: l, live: v, save: s } = latest.current;
      if (v && "split" in v) s({ ...l, split: v.split });
      else if (v) s({ ...l, heights: { ...(l.heights ?? {}), [v.id]: v.h } });
      setLive(null);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", up);
  };

  const setHeight = (id: string, h: number | null) => {
    const next = { ...heights };
    if (h === null) delete next[id];
    else next[id] = clampHeight(h);
    save({ ...layout, heights: next });
  };

  const column = (side: Side) => (
    <div data-column={side} className="flex min-h-0 min-w-0 flex-col gap-3">
      {layout[side].map((id) => {
        const h = heightOf(id);
        const fill = FILLS.has(id) && h === undefined;
        return (
          <div key={id} data-widget={id} className={`relative flex flex-col ${fill ? "min-h-[180px] flex-1" : "shrink-0"} ${dragging?.id === id ? "opacity-40" : ""}`}>
            {over?.side === side && over.before === id && dragging?.id !== id && <div aria-hidden="true" className="absolute -top-2 left-0 right-0 h-0.5 rounded-full bg-text" />}
            {editing && (
              <div className="flex h-7 shrink-0 items-center gap-1 text-[11px] text-text-3">
                <button
                  type="button"
                  aria-label={`Move ${WIDGETS[id]}`}
                  title="Drag, or use the arrow keys"
                  onPointerDown={(e) => {
                    e.preventDefault();
                    setDragging({ id, x: e.clientX, y: e.clientY });
                  }}
                  onKeyDown={(e: KeyboardEvent) => {
                    const dir = ARROWS[e.key];
                    if (!dir) return;
                    e.preventDefault();
                    refocus.current = id;
                    save(step(layout, id, dir));
                  }}
                  className="flex h-6 cursor-grab items-center gap-1.5 rounded-[6px] pl-1 pr-2 hover:bg-card hover:text-text focus-visible:bg-card focus-visible:text-text focus-visible:outline-none"
                >
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor" aria-hidden="true">
                    {[3, 6, 9].flatMap((y) => [4, 8].map((x) => <circle key={`${x}-${y}`} cx={x} cy={y} r="1" />))}
                  </svg>
                  {WIDGETS[id]}
                </button>
                {h !== undefined && <span className="font-mono text-[10px] text-text-3">{`${h} px`}</span>}
                <button type="button" aria-label={`Hide ${WIDGETS[id]}`} onClick={() => save({ ...layout, left: layout.left.filter((x) => x !== id), right: layout.right.filter((x) => x !== id) })} className="ml-auto h-6 rounded-[6px] px-2 hover:bg-card hover:text-text">
                  Hide
                </button>
              </div>
            )}
            <div
              data-box
              style={h !== undefined ? { height: h } : undefined}
              className={h !== undefined ? "flex shrink-0 flex-col overflow-hidden [&>*]:min-h-0 [&>*]:flex-1 [&>*]:overflow-y-auto" : fill ? "flex min-h-0 flex-1 flex-col [&>*:last-child]:min-h-0 [&>*:last-child]:flex-1" : ""}
            >
              {render(id)}
            </div>
            {editing && (
              <div
                role="separator"
                aria-orientation="horizontal"
                aria-label={`Resize ${WIDGETS[id]}`}
                aria-valuenow={h}
                aria-valuemin={HEIGHT_MIN}
                tabIndex={0}
                title="Drag to resize, double-click to fit"
                onPointerDown={(e) => {
                  e.preventDefault();
                  const box = (e.currentTarget.previousElementSibling as HTMLElement | null)?.offsetHeight ?? 0;
                  const y0 = e.clientY;
                  const h0 = h ?? Math.max(HEIGHT_MIN, box);
                  setLive({ id, h: h0 });
                  track((m) => setLive({ id, h: clampHeight(h0 + m.clientY - y0) }));
                }}
                onDoubleClick={() => setHeight(id, null)}
                onKeyDown={(e) => {
                  if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
                  e.preventDefault();
                  const box = (e.currentTarget.previousElementSibling as HTMLElement | null)?.offsetHeight ?? 0;
                  const now = h ?? Math.max(HEIGHT_MIN, box);
                  setHeight(id, now + (e.key === "ArrowDown" ? 20 : -20));
                }}
                className="group absolute -bottom-2.5 left-0 right-0 z-10 flex h-3 cursor-row-resize items-center justify-center focus-visible:outline-none"
              >
                <span className="h-1 w-10 rounded-full bg-line-strong transition-colors group-hover:bg-text-2 group-focus-visible:bg-text" />
              </div>
            )}
          </div>
        );
      })}
      {over?.side === side && over.before === null && <div aria-hidden="true" className="h-0.5 shrink-0 rounded-full bg-text" />}
      {editing && (
        <div className="flex h-12 shrink-0 items-center justify-center rounded-panel border border-dashed border-line text-[11px] text-text-3" aria-hidden="true">
          {layout[side].length === 0 ? "Drop a widget here" : ""}
        </div>
      )}
    </div>
  );

  return (
    <>
      <div
        ref={grid}
        style={{ gridTemplateColumns: `minmax(0, ${split}fr) minmax(0, ${100 - split}fr)` }}
        className={`relative grid min-h-0 flex-1 gap-3 ${dragging || live ? "select-none" : ""} ${live && "split" in live ? "cursor-col-resize" : ""}`}
      >
        {column("left")}
        {column("right")}
        {editing && (
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label="Column width"
            aria-valuenow={split}
            aria-valuemin={SPLIT_MIN}
            aria-valuemax={SPLIT_MAX}
            tabIndex={0}
            title="Drag to set the column width, double-click for half and half"
            style={{ left: `calc((100% - 12px) * ${split / 100})` }}
            onPointerDown={(e) => {
              e.preventDefault();
              const rect = grid.current?.getBoundingClientRect();
              if (!rect || rect.width <= 0) return;
              setLive({ split });
              track((m) => setLive({ split: clampSplit(((m.clientX - rect.left) / rect.width) * 100) }));
            }}
            onDoubleClick={() => save({ ...layout, split: 50 })}
            onKeyDown={(e) => {
              if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
              e.preventDefault();
              save({ ...layout, split: clampSplit(split + (e.key === "ArrowRight" ? 5 : -5)) });
            }}
            className="group absolute bottom-0 top-0 z-10 flex w-3 cursor-col-resize justify-center focus-visible:outline-none"
          >
            <span className="w-0.5 rounded-full bg-transparent transition-colors group-hover:bg-text-2 group-focus-visible:bg-text" />
          </div>
        )}
      </div>
      {dragging && (
        <div aria-hidden="true" style={{ left: dragging.x + 12, top: dragging.y + 12 }} className="pointer-events-none fixed z-50 rounded-control bg-raised px-2 py-1 text-[12px] text-text shadow-menu">
          {WIDGETS[dragging.id]}
        </div>
      )}
    </>
  );
}
