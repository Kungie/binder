import { inTauri } from "./api";

const CONTROLS = "button, a, input, select, textarea, label, summary, [role=button], [role=tab], [role=menuitem], [contenteditable=true], [data-no-drag]";

/** Whether a press here should move the window: inside a drag region, and not on a control or inside one. */
export function dragTarget(el: Element | null): boolean {
  if (!el?.closest("[data-tauri-drag-region]")) return false;
  return !el.closest(CONTROLS);
}

/** Lets a title bar be held anywhere but its controls. Tauri's own handler only takes presses on the region itself,
 *  not on the text inside it. */
export function enableWindowDrag(): () => void {
  if (!inTauri) return () => {};
  const down = (e: MouseEvent) => {
    if (e.button !== 0 || e.detail > 1) return;
    const el = e.target as Element | null;
    // A press on the region itself is Tauri's to handle.
    if (el?.hasAttribute("data-tauri-drag-region") || !dragTarget(el)) return;
    e.preventDefault();
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().startDragging());
  };
  document.addEventListener("mousedown", down);
  return () => document.removeEventListener("mousedown", down);
}
