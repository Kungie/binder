import { describe, expect, it } from "vitest";
import { demoSeed, memoryApi } from "./memory";
import type { Transfer } from "./types";

const settle = () => new Promise((r) => setTimeout(r, 20));

describe("memory Cloud", () => {
  it("upload finishes after two progress events and lands in the folder", async () => {
    const api = memoryApi(demoSeed());
    const seen: Transfer[] = [];
    api.onTransfer((t) => seen.push(t));
    const [id] = await api.cloudUpload(["/Users/x/Desktop/clip.mov"], "Videos", "keep_both");
    await settle();
    expect(seen.map((t) => t.state)).toEqual(["running", "running", "done"]);
    expect(seen.every((t) => t.id === id)).toBe(true);
    expect((await api.cloudList("Videos", false)).map((e) => e.name)).toContain("clip.mov");
  });

  it("keep both names the second copy with a 2", async () => {
    const api = memoryApi(demoSeed());
    await api.cloudUpload(["/tmp/lease.pdf"], "Documents", "keep_both");
    await settle();
    expect((await api.cloudList("Documents", false)).map((e) => e.name)).toEqual(["lease 2.pdf", "lease.pdf", "taxes-2025.xlsx"]);
  });

  it("a cancelled upload adds nothing", async () => {
    const api = memoryApi(demoSeed());
    const seen: Transfer[] = [];
    api.onTransfer((t) => seen.push(t));
    const [id] = await api.cloudUpload(["/tmp/big.iso"], "", "keep_both");
    await api.cloudCancel(id);
    await settle();
    expect(seen.at(-1)?.state).toBe("cancelled");
    expect((await api.cloudList("", false)).map((e) => e.name)).not.toContain("big.iso");
  });

  it("trash then restore puts a folder back with its files", async () => {
    const api = memoryApi(demoSeed());
    await api.cloudTrash("Photos");
    expect((await api.cloudList("", false)).map((e) => e.name)).not.toContain("Photos");
    const [item] = await api.cloudTrashList();
    expect(item.origin).toBe("Photos");
    await api.cloudRestore(item.id);
    expect((await api.cloudList("Photos/2026", false)).map((e) => e.name)).toEqual(["lake.jpg", "workshop.heic"]);
    expect(await api.cloudTrashList()).toEqual([]);
  });

  it("hides dotfiles unless asked and lists folders first", async () => {
    const api = memoryApi(demoSeed());
    const names = (await api.cloudList("", false)).map((e) => e.name);
    expect(names).toEqual(["Backups", "Documents", "Photos", "Videos", "README.md"]);
    expect((await api.cloudList("", true)).map((e) => e.name)).toContain(".DS_Store");
  });

  it("refuses everything when the share isn't mounted", async () => {
    const api = memoryApi(demoSeed(), { cloudMounted: false });
    expect((await api.cloudStatus()).mounted).toBe(false);
    await expect(api.cloudList("", false)).rejects.toThrow("Cloud not connected.");
    await expect(api.cloudUpload(["/tmp/a"], "", "skip")).rejects.toThrow("Cloud not connected.");
  });

  it("connect needs the share address", async () => {
    const api = memoryApi(demoSeed());
    await expect(api.cloudConnect()).rejects.toThrow("Set the share address in Settings.");
  });
});

describe("memory GitHub sync", () => {
  it("adds new repos, links hand projects by name and keeps known ones", async () => {
    const api = memoryApi(demoSeed());
    const report = await api.githubSync();
    expect(report).toEqual({ added: 2, linked: 1, updated: 0, archived: 0 });
    const projects = await api.listProjects();
    expect(projects.find((p) => p.name === "homelab")?.github).toBe("demo-dev/homelab");
    expect(projects.find((p) => p.name === "pixel-clock")?.state).toBe("paused");
    expect(projects.find((p) => p.name === "dotfiles")?.state).toBe("active");
  });

  it("signed out says how to sign in", async () => {
    const api = memoryApi({ ...demoSeed(), github: null });
    await expect(api.githubSync()).rejects.toThrow("Sign in with gh auth login");
  });
});
