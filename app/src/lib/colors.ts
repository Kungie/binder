/** Mirrors crates/core/src/projects.rs: the project palette and a stable colour per name (FNV-1a). */
export const PALETTE = ["#5E6AD2", "#26B5CE", "#4CB782", "#F2C94C", "#F2994A", "#EB5757", "#B76EDB", "#8D8D8D"] as const;

/** What each palette colour is called, for the picker's labels. */
export const COLOR_NAMES: Record<(typeof PALETTE)[number], string> = {
  "#5E6AD2": "Indigo",
  "#26B5CE": "Cyan",
  "#4CB782": "Green",
  "#F2C94C": "Yellow",
  "#F2994A": "Orange",
  "#EB5757": "Red",
  "#B76EDB": "Purple",
  "#8D8D8D": "Grey",
};

export function colorFor(name: string): string {
  let h = 0x811c9dc5;
  for (const b of new TextEncoder().encode(name.toLowerCase())) {
    h ^= b;
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return PALETTE[h % PALETTE.length];
}
