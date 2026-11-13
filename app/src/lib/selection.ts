/** Which rows are selected. `items` ends with the row the keyboard or the last click is on; `anchor` is where a
 * shift-range starts. */
export type Selection = { items: string[]; anchor: string | null };

export const empty: Selection = { items: [], anchor: null };

const head = (sel: Selection) => sel.items[sel.items.length - 1] ?? null;

/** The rows from `from` to `to` in screen order, ending at `to`. */
function range(order: string[], from: string, to: string): string[] {
  const a = order.indexOf(from);
  const b = order.indexOf(to);
  if (a < 0 || b < 0) return [to];
  const step = b >= a ? 1 : -1;
  const out: string[] = [];
  for (let i = a; i !== b + step; i += step) out.push(order[i]);
  return out;
}

export function click(sel: Selection, rel: string, order: string[], mods: { meta: boolean; shift: boolean }): Selection {
  if (mods.shift && sel.anchor) return { items: range(order, sel.anchor, rel), anchor: sel.anchor };
  if (mods.meta) {
    if (sel.items.includes(rel)) {
      const items = sel.items.filter((r) => r !== rel);
      return { items, anchor: items.length ? (sel.anchor === rel ? items[items.length - 1] : sel.anchor) : null };
    }
    return { items: [...sel.items, rel], anchor: rel };
  }
  return { items: [rel], anchor: rel };
}

export function all(order: string[]): Selection {
  return { items: [...order], anchor: order[0] ?? null };
}

/** Moves the keyboard row by `delta`; with `extend` the range from the anchor grows or shrinks to it. */
export function move(sel: Selection, order: string[], delta: number, extend: boolean): Selection {
  if (!order.length) return empty;
  const at = head(sel);
  const i = at === null ? -1 : order.indexOf(at);
  const next = order[i < 0 ? (delta > 0 ? 0 : order.length - 1) : Math.max(0, Math.min(order.length - 1, i + delta))];
  if (extend && sel.anchor) return { items: range(order, sel.anchor, next), anchor: sel.anchor };
  return { items: [next], anchor: next };
}
