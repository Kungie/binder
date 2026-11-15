import { describe, expect, it } from "vitest";
import { demoSeed, memoryApi } from "./memory";

describe("memory brief", () => {
  it("brief_lists_demo_items_and_dismiss_clears_them", async () => {
    const api = memoryApi(demoSeed());
    const b = await api.brief();
    expect(b.shown).toBe(true);
    const kinds = new Set(b.items.map((i) => i.kind));
    expect(kinds).toEqual(new Set(["run", "container", "server_restart", "print", "issue"]));
    await api.dismissBrief();
    const after = await api.brief();
    expect(after.shown).toBe(false);
    expect(after.items).toEqual([]);
  });
});
