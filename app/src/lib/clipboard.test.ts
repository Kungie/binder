import { describe, expect, it } from "vitest";
import { terminalClipboard } from "./clipboard";

describe("terminal clipboard", () => {
  it("never hands the Mac clipboard to the server", async () => {
    const provider = terminalClipboard(() => {});
    expect(await provider.readText("c")).toBe("");
  });

  it("copies what the server sends through the app", async () => {
    const copied: string[] = [];
    const provider = terminalClipboard((t) => copied.push(t));
    await provider.writeText("c", "git status");
    expect(copied).toEqual(["git status"]);
  });

  it("an empty or unreadable write leaves the clipboard alone", async () => {
    const copied: string[] = [];
    await terminalClipboard((t) => copied.push(t)).writeText("c", "");
    expect(copied).toEqual([]);
  });
});
