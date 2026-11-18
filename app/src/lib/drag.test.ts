import { describe, expect, it } from "vitest";
import { dragTarget } from "./drag";

describe("dragTarget", () => {
  it("text inside a drag region drags the window, controls don't", () => {
    document.body.innerHTML = `<header data-tauri-drag-region><h1 id="t">Agents</h1><span id="s"><b id="b">x</b></span><button id="btn"><span id="in">Go</span></button><input id="i" /></header><main><p id="out">body</p></main>`;
    const el = (id: string) => document.getElementById(id)!;
    expect(dragTarget(el("t"))).toBe(true);
    expect(dragTarget(el("b"))).toBe(true);
    expect(dragTarget(el("btn"))).toBe(false);
    expect(dragTarget(el("in"))).toBe(false);
    expect(dragTarget(el("i"))).toBe(false);
    expect(dragTarget(el("out"))).toBe(false);
  });
});
