import { describe, expect, it } from "vitest";
import { all, click, move, type Selection } from "./selection";

const order = ["a", "b", "c", "d", "e"];
const none: Selection = { items: [], anchor: null };
const plain = { meta: false, shift: false };

describe("selection", () => {
  it("a plain click selects one", () => {
    expect(click(click(none, "b", order, plain), "d", order, plain)).toEqual({ items: ["d"], anchor: "d" });
  });

  it("meta toggles", () => {
    let s = click(none, "b", order, plain);
    s = click(s, "d", order, { meta: true, shift: false });
    expect([...s.items].sort()).toEqual(["b", "d"]);
    s = click(s, "b", order, { meta: true, shift: false });
    expect(s.items).toEqual(["d"]);
  });

  it("shift selects a range from the anchor", () => {
    let s = click(none, "b", order, plain);
    s = click(s, "d", order, { meta: false, shift: true });
    expect([...s.items].sort()).toEqual(["b", "c", "d"]);
    expect(s.anchor).toBe("b");
    s = click(s, "a", order, { meta: false, shift: true });
    expect([...s.items].sort()).toEqual(["a", "b"]);
  });

  it("arrows move and shift-arrows extend", () => {
    let s = click(none, "b", order, plain);
    s = move(s, order, 1, false);
    expect(s).toEqual({ items: ["c"], anchor: "c" });
    s = move(s, order, 1, true);
    s = move(s, order, 1, true);
    expect([...s.items].sort()).toEqual(["c", "d", "e"]);
    s = move(s, order, 1, true);
    expect([...s.items].sort()).toEqual(["c", "d", "e"]);
    s = move(s, order, -1, true);
    expect([...s.items].sort()).toEqual(["c", "d"]);
    expect(move(none, order, 1, false).items).toEqual(["a"]);
  });

  it("all", () => {
    expect([...all(order).items].sort()).toEqual(order);
    expect(all([])).toEqual({ items: [], anchor: null });
  });
});
