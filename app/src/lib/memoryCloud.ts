import type { Dropped } from "./api";
import type { AppChoice, CloudEntry, CloudInfo, CloudStatus, Conflict, Space, Transfer, TrashEntry } from "./types";

/** What the fake Cloud starts with: every entry by its path, folders included. */
export interface CloudSeed {
  entries: CloudEntry[];
  trash?: TrashEntry[];
  /** What each zip holds, paths relative to the zip's folder. */
  zips?: Record<string, CloudEntry[]>;
}

const SD: Space = { total: 29e9, free: 21e9 };

const parentOf = (rel: string) => (rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "");
const baseOf = (path: string) => path.split("/").filter(Boolean).pop() ?? path;
const join = (dir: string, name: string) => (dir ? `${dir}/${name}` : name);
const under = (rel: string, dir: string) => rel === dir || rel.startsWith(`${dir}/`);

function checkedName(name: string): string {
  const n = name.trim();
  if (!n || n === "." || n === ".." || n.includes("/")) throw new Error("A name can't be empty, . or .., or contain /.");
  return n;
}

/** Cloud as the core does it, over an in-memory tree. Uploads finish after two progress events. */
export function memoryCloud(seed: CloudSeed | undefined, opts: { mounted: () => boolean; share: () => string | null }) {
  let entries = (seed?.entries ?? []).map((e) => ({ ...e }));
  let trash: (TrashEntry & { children: CloudEntry[] })[] = (seed?.trash ?? []).map((t) => ({ ...t, children: [] }));
  const zips: Record<string, CloudEntry[]> = Object.fromEntries(Object.entries(seed?.zips ?? {}).map(([k, v]) => [k, v.map((e) => ({ ...e }))]));
  const quickLooked: string[] = [];
  const transferListeners = new Set<(t: Transfer) => void>();
  const dropListeners = new Set<(d: Dropped) => void>();
  const dragged: string[][] = [];
  const cancelled = new Set<number>();
  const opened: string[] = [];
  let connects = 0;
  let nextTransfer = 1;
  const now = () => Math.floor(Date.now() / 1000);

  const mounted = () => {
    if (!opts.mounted()) throw new Error("Cloud not connected.");
  };
  const find = (rel: string) => entries.find((e) => e.rel === rel);
  const get = (rel: string) => {
    const e = find(rel);
    if (!e) throw new Error(`Nothing at ${rel}.`);
    return e;
  };
  const folder = (rel: string) => {
    if (rel && get(rel).kind !== "dir") throw new Error("That's not a folder.");
  };
  /** As the core: a taken folder never splits at its dot. */
  const split = (dir: string, name: string) => {
    const dot = name.lastIndexOf(".");
    return dot > 0 && find(join(dir, name))?.kind !== "dir" ? [name.slice(0, dot), name.slice(dot)] : [name, ""];
  };
  const freeName = (dir: string, name: string) => {
    if (!find(join(dir, name))) return name;
    const [stem, ext] = split(dir, name);
    for (let n = 2; ; n++) if (!find(join(dir, `${stem} ${n}${ext}`))) return `${stem} ${n}${ext}`;
  };
  const copyName = (dir: string, name: string) => {
    const [stem, ext] = split(dir, name);
    if (!find(join(dir, `${stem} copy${ext}`))) return `${stem} copy${ext}`;
    for (let n = 2; ; n++) if (!find(join(dir, `${stem} copy ${n}${ext}`))) return `${stem} copy ${n}${ext}`;
  };
  /** A copy of an entry and everything under it at a new path. */
  const clone = (from: string, to: string) => {
    const copies = entries.filter((e) => under(e.rel, from)).map((e) => ({ ...e, rel: to + e.rel.slice(from.length), modified: now() }));
    copies.forEach((e) => (e.name = baseOf(e.rel)));
    entries.push(...copies);
  };
  const toTrash = (taken: CloudEntry) => {
    const children = entries.filter((e) => e !== taken && under(e.rel, taken.rel));
    entries = entries.filter((e) => !under(e.rel, taken.rel));
    trash.unshift({ id: `${Date.now()}-${trash.length}`, origin: taken.rel, deleted: now(), unfinished: false, entry: { ...taken }, children });
  };
  const sizeOf = (rel: string): [number, number] => {
    const e = get(rel);
    if (e.kind !== "dir") return [e.size, 1];
    const inside = entries.filter((x) => x !== e && under(x.rel, rel));
    return [inside.reduce((s, x) => s + x.size, 0), inside.length];
  };
  /** Moves an entry and everything under it to a new path. */
  const relocate = (from: string, to: string) => {
    for (const e of entries) {
      if (!under(e.rel, from)) continue;
      e.rel = to + e.rel.slice(from.length);
      e.name = baseOf(e.rel);
    }
  };
  const emit = (t: Transfer) => transferListeners.forEach((cb) => cb({ ...t }));
  /** Two progress events, then the end, each a tick apart. `land` runs only when it finishes. */
  const transfer = (name: string, total: number, land: () => void, unit: Transfer["unit"] = "bytes") => {
    const id = nextTransfer++;
    const t: Transfer = { id, name, unit, done: 0, total, state: "running", error: null };
    const steps = [total / 2, total];
    const step = (i: number) =>
      setTimeout(() => {
        if (cancelled.has(id)) return emit({ ...t, state: "cancelled" });
        if (i < steps.length) {
          t.done = steps[i];
          emit(t);
          return step(i + 1);
        }
        land();
        emit({ ...t, state: "done" });
      }, 0);
    step(0);
    return id;
  };

  return {
    cloudOpened: opened,
    quickLooked,
    get cloudConnects() {
      return connects;
    },
    /** Acts as if files were dropped on the window from Finder. */
    dropFiles(paths: string[], x = 0, y = 0) {
      dropListeners.forEach((cb) => cb({ paths, x, y }));
    },
    dragged,
    async startDrag(paths: string[]) {
      dragged.push([...paths]);
    },
    onDragOver(_cb: (p: { x: number; y: number } | null) => void) {
      return () => {};
    },
    async cloudStatus(): Promise<CloudStatus> {
      const on = opts.mounted();
      const used = entries.reduce((s, e) => s + e.size, 0);
      const total = 2e12;
      return { mounted: on, path: "/Volumes/cloud", space: on ? { total, free: total - used - 600e9 } : null };
    },
    async cloudList(rel: string, showHidden: boolean): Promise<CloudEntry[]> {
      mounted();
      folder(rel);
      return entries
        .filter((e) => parentOf(e.rel) === rel && (showHidden || !e.name.startsWith(".")))
        .sort((a, b) => Number(a.kind !== "dir") - Number(b.kind !== "dir") || a.name.toLowerCase().localeCompare(b.name.toLowerCase()))
        .map((e) => ({ ...e }));
    },
    async cloudMkdir(relDir: string, name: string) {
      mounted();
      folder(relDir);
      const n = checkedName(name);
      if (find(join(relDir, n))) throw new Error(`There is already something called ${n} here.`);
      entries.push({ name: n, rel: join(relDir, n), kind: "dir", size: 0, modified: now() });
    },
    async cloudRename(rel: string, newName: string) {
      mounted();
      get(rel);
      const n = checkedName(newName);
      const to = join(parentOf(rel), n);
      if (find(to)) throw new Error(`There is already something called ${n} here.`);
      relocate(rel, to);
    },
    async cloudMove(rel: string, destDirRel: string) {
      mounted();
      get(rel);
      folder(destDirRel);
      if (under(destDirRel, rel)) throw new Error("A folder can't go inside itself.");
      relocate(rel, join(destDirRel, freeName(destDirRel, baseOf(rel))));
      return [] as number[];
    },
    async cloudTrash(rel: string) {
      mounted();
      if (!rel) throw new Error("That can't go to the trash.");
      const entry = get(rel);
      const children = entries.filter((e) => e !== entry && under(e.rel, rel));
      entries = entries.filter((e) => !under(e.rel, rel));
      trash.unshift({ id: `${Date.now()}-${trash.length}`, origin: rel, deleted: now(), unfinished: false, entry: { ...entry }, children });
    },
    async cloudTrashList(): Promise<TrashEntry[]> {
      mounted();
      return trash.map(({ children: _, ...t }) => ({ ...t, entry: { ...t.entry } }));
    },
    async cloudRestore(id: string) {
      mounted();
      const item = trash.find((t) => t.id === id);
      if (!item) throw new Error("That's no longer in the trash.");
      if (item.unfinished) throw new Error("An unfinished upload can't be restored.");
      const dir = parentOf(item.origin);
      // Folders that went missing come back as plain folders.
      dir.split("/").reduce((acc, part) => {
        const r = join(acc, part);
        if (part && !find(r)) entries.push({ name: part, rel: r, kind: "dir", size: 0, modified: now() });
        return r;
      }, "");
      const to = join(dir, freeName(dir, item.entry.name));
      const moved = [item.entry, ...item.children].map((e) => ({ ...e, rel: to + e.rel.slice(item.origin.length), name: "" }));
      moved.forEach((e) => (e.name = baseOf(e.rel)));
      entries.push(...moved);
      trash = trash.filter((t) => t !== item);
    },
    async cloudEmptyTrash() {
      mounted();
      trash = [];
    },
    async cloudOpen(rel: string): Promise<number[]> {
      mounted();
      const e = get(rel);
      if (e.kind === "file" && rel.toLowerCase().endsWith(".zip")) return this.cloudExtract(rel);
      opened.push(rel);
      return [];
    },
    async cloudCopy(rels: string[], destDirRel: string, conflict: Conflict): Promise<number[]> {
      mounted();
      folder(destDirRel);
      return rels.map((rel) => {
        const e = get(rel);
        if (e.kind === "dir" && under(destDirRel, rel)) throw new Error("A folder can't go inside itself.");
        return transfer(e.name, e.size || 1, () => {
          const taken = find(join(destDirRel, e.name));
          if (taken && conflict === "skip") return;
          if (taken && conflict === "replace") toTrash(taken);
          clone(rel, join(destDirRel, taken && conflict === "keep_both" ? freeName(destDirRel, e.name) : e.name));
        });
      });
    },
    async cloudDuplicate(rels: string[]): Promise<number[]> {
      mounted();
      return rels.map((rel) => {
        const e = get(rel);
        const dir = parentOf(rel);
        const name = copyName(dir, e.name);
        return transfer(name, e.size || 1, () => clone(rel, join(dir, freeName(dir, name))));
      });
    },
    async cloudCompress(rels: string[]): Promise<number[]> {
      mounted();
      if (!rels.length) throw new Error("Nothing to compress.");
      const dir = parentOf(rels[0]);
      if (rels.some((r) => parentOf(r) !== dir)) throw new Error("Compress items from one folder.");
      const items = rels.map(get);
      const name = items.length === 1 ? `${items[0].name}.zip` : "Archive.zip";
      const inside = entries.filter((e) => rels.some((r) => under(e.rel, r))).map((e) => ({ ...e, rel: dir ? e.rel.slice(dir.length + 1) : e.rel }));
      return [
        transfer(
          name,
          inside.length,
          () => {
            const final = join(dir, freeName(dir, name));
            entries.push({ name: baseOf(final), rel: final, kind: "file", size: inside.reduce((s, e) => s + e.size, 0) * 0.6, modified: now() });
            zips[final] = inside;
          },
          "items",
        ),
      ];
    },
    async cloudExtract(rel: string): Promise<number[]> {
      mounted();
      const zip = get(rel);
      const dir = parentOf(rel);
      const stem = zip.name.replace(/\.[^.]*$/, "");
      const inside = zips[rel] ?? [];
      return [
        transfer(
          zip.name,
          inside.length || 1,
          () => {
            const to = join(dir, freeName(dir, stem));
            entries.push({ name: baseOf(to), rel: to, kind: "dir", size: 0, modified: now() });
            entries.push(...inside.map((e) => ({ ...e, rel: join(to, e.rel), modified: now() })));
          },
          "items",
        ),
      ];
    },
    async cloudInfo(rel: string): Promise<CloudInfo> {
      mounted();
      const entry = get(rel);
      return { entry: { ...entry }, created: entry.modified, path: `/Volumes/cloud/${rel}` };
    },
    async cloudSize(rel: string): Promise<[number, number]> {
      mounted();
      return sizeOf(rel);
    },
    async cloudSpace(rel: string): Promise<Space> {
      mounted();
      if (under(rel, "SD")) return { ...SD };
      const used = entries.reduce((s, e) => s + e.size, 0);
      return { total: 2e12, free: 2e12 - used - 600e9 };
    },
    async cloudFastPath(): Promise<"ssh" | "smb"> {
      mounted();
      return "ssh";
    },
    async cloudQuickLook(rel: string) {
      mounted();
      get(rel);
      quickLooked.push(rel);
    },
    async cloudAppsFor(rel: string): Promise<AppChoice[]> {
      mounted();
      get(rel);
      return [
        { name: "Preview", path: "/System/Applications/Preview.app", default: true },
        { name: "TextEdit", path: "/System/Applications/TextEdit.app", default: false },
      ];
    },
    async cloudOpenWith(rel: string, app: string) {
      mounted();
      get(rel);
      opened.push(`${rel} with ${app}`);
    },
    async cloudConnect() {
      if (!opts.share()) throw new Error("Set the share address in Settings.");
      connects++;
    },
    async cloudUpload(paths: string[], destRel: string, conflict: Conflict): Promise<number[]> {
      mounted();
      folder(destRel);
      return paths.map((p) => {
        const name = baseOf(p);
        const size = 1024 ** 2 * (name.length + 1);
        return transfer(name, size, () => {
          const taken = find(join(destRel, name));
          if (taken && conflict === "skip") return;
          // Like the core: the replaced item goes to the trash, never away outright.
          if (taken && conflict === "replace") toTrash(taken);
          const final = taken && conflict === "keep_both" ? freeName(destRel, name) : name;
          entries.push({ name: final, rel: join(destRel, final), kind: "file", size, modified: now() });
        });
      });
    },
    async cloudDownload(rels: string[], _destDir: string, _conflict: Conflict): Promise<number[]> {
      mounted();
      return rels.map((r) => {
        const e = get(r);
        return transfer(e.name, e.size || 1, () => {});
      });
    },
    async cloudCancel(id: number) {
      cancelled.add(id);
    },
    onTransfer(cb: (t: Transfer) => void) {
      transferListeners.add(cb);
      return () => void transferListeners.delete(cb);
    },
    onDrop(cb: (d: Dropped) => void) {
      dropListeners.add(cb);
      return () => void dropListeners.delete(cb);
    },
  };
}
