import type { CloudEntry, CloudStatus, Conflict, Transfer, TrashEntry } from "./types";

/** What the fake Cloud starts with: every entry by its path, folders included. */
export interface CloudSeed {
  entries: CloudEntry[];
  trash?: TrashEntry[];
}

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
  const transferListeners = new Set<(t: Transfer) => void>();
  const dropListeners = new Set<(paths: string[]) => void>();
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
  const freeName = (dir: string, name: string) => {
    if (!find(join(dir, name))) return name;
    const dot = name.lastIndexOf(".");
    const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, ""];
    for (let n = 2; ; n++) if (!find(join(dir, `${stem} ${n}${ext}`))) return `${stem} ${n}${ext}`;
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
  const transfer = (name: string, total: number, land: () => void) => {
    const id = nextTransfer++;
    const t: Transfer = { id, name, done: 0, total, state: "running", error: null };
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
    get cloudConnects() {
      return connects;
    },
    /** Acts as if files were dropped on the window from Finder. */
    dropFiles(paths: string[]) {
      dropListeners.forEach((cb) => cb(paths));
    },
    async cloudStatus(): Promise<CloudStatus> {
      const on = opts.mounted();
      const used = entries.reduce((s, e) => s + e.size, 0);
      const total = 2 * 1024 ** 4;
      return { mounted: on, path: "/Volumes/cloud", space: on ? { total, free: total - used - 600 * 1024 ** 3 } : null };
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
    },
    async cloudTrash(rel: string) {
      mounted();
      if (!rel) throw new Error("That can't go to the trash.");
      const entry = get(rel);
      const children = entries.filter((e) => e !== entry && under(e.rel, rel));
      entries = entries.filter((e) => !under(e.rel, rel));
      trash.unshift({ id: `${Date.now()}-${trash.length}`, origin: rel, deleted: now(), entry: { ...entry }, children });
    },
    async cloudTrashList(): Promise<TrashEntry[]> {
      mounted();
      return trash.map(({ children: _, ...t }) => ({ ...t, entry: { ...t.entry } }));
    },
    async cloudRestore(id: string) {
      mounted();
      const item = trash.find((t) => t.id === id);
      if (!item) throw new Error("That's no longer in the trash.");
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
    async cloudOpen(rel: string) {
      mounted();
      get(rel);
      opened.push(rel);
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
          if (taken && conflict === "replace") entries = entries.filter((e) => !under(e.rel, taken.rel));
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
    onDrop(cb: (paths: string[]) => void) {
      dropListeners.add(cb);
      return () => void dropListeners.delete(cb);
    },
  };
}
