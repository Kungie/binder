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
    expect(seen[seen.length - 1]?.state).toBe("cancelled");
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
    expect(report).toEqual({ added: 1, linked: 1, updated: 0, archived: 0 });
    const projects = await api.listProjects();
    expect(projects.find((p) => p.name === "homelab")?.github).toBe("demo-dev/homelab");
    expect(projects.find((p) => p.name === "weather-cli")?.state).toBe("active");
    expect(projects.find((p) => p.name === "old-site")?.archived).toBe(true);
  });

  it("signed out says how to sign in", async () => {
    const api = memoryApi({ ...demoSeed(), github: null });
    await expect(api.githubSync()).rejects.toThrow("Sign in with gh auth login");
  });
});

describe("memory Cloud replace", () => {
  it("replace puts the old file in the trash", async () => {
    const api = memoryApi(demoSeed());
    await api.cloudUpload(["/tmp/README.md"], "", "replace");
    await settle();
    expect((await api.cloudTrashList()).map((t) => t.origin)).toEqual(["README.md"]);
    expect((await api.cloudList("", false)).filter((e) => e.name === "README.md")).toHaveLength(1);
  });
});

describe("memory Cloud names agree with the core", () => {
  const list = async (api: ReturnType<typeof memoryApi>, rel: string) => (await api.cloudList(rel, false)).map((e) => `${e.name}${e.kind === "dir" ? "/" : ""}`);

  it("duplicate names like Finder", async () => {
    const api = memoryApi(demoSeed());
    await api.cloudDuplicate(["Documents/lease.pdf"]);
    await settle();
    await api.cloudDuplicate(["Documents/lease.pdf"]);
    await api.cloudDuplicate(["Photos"]);
    await settle();
    expect(await list(api, "Documents")).toEqual(["lease copy 2.pdf", "lease copy.pdf", "lease.pdf", "taxes-2025.xlsx"]);
    expect(await list(api, "Photos copy")).toEqual(["2026/"]);
    expect(await list(api, "Photos copy/2026")).toEqual(["lake.jpg", "workshop.heic"]);
  });

  it("copy keeps both on a clash", async () => {
    const api = memoryApi(demoSeed());
    await api.cloudCopy(["README.md"], "Documents", "keep_both");
    await api.cloudCopy(["README.md"], "", "keep_both");
    await settle();
    expect(await list(api, "Documents")).toContain("README.md");
    expect(await list(api, "")).toContain("README 2.md");
  });

  it("compress and extract", async () => {
    const api = memoryApi(demoSeed());
    const seen: Transfer[] = [];
    api.onTransfer((t) => seen.push(t));
    await api.cloudCompress(["Documents/lease.pdf"]);
    await api.cloudCompress(["Documents/lease.pdf", "Documents/taxes-2025.xlsx"]);
    await api.cloudCompress(["Documents/lease.pdf", "Documents/taxes-2025.xlsx"]);
    await settle();
    expect(seen.every((t) => t.unit === "items")).toBe(true);
    expect(await list(api, "Documents")).toEqual(["Archive 2.zip", "Archive.zip", "lease.pdf", "lease.pdf.zip", "taxes-2025.xlsx"]);
    await api.cloudExtract("Documents/Archive.zip");
    await settle();
    expect(await list(api, "Documents/Archive")).toEqual(["lease.pdf", "taxes-2025.xlsx"]);
    await api.cloudExtract("Documents/lease.pdf.zip");
    await settle();
    // The stem is taken by a file, so the core's free name splits at its dot.
    expect(await list(api, "Documents/lease 2.pdf")).toEqual(["lease.pdf"]);
  });

  it("info, size and the open of a zip", async () => {
    const api = memoryApi(demoSeed());
    const info = await api.cloudInfo("Photos/2026/lake.jpg");
    expect(info.path).toBe("/Volumes/cloud/Photos/2026/lake.jpg");
    expect(info.entry.name).toBe("lake.jpg");
    expect(await api.cloudSize("Photos")).toEqual([9 * 1024 ** 2, 3]);
    const ids = await api.cloudOpen("Backups/old-site.zip");
    expect(ids).toHaveLength(1);
    await settle();
    expect(await list(api, "Backups/old-site")).toEqual(["index.html"]);
  });
});
