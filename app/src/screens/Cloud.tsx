import { type KeyboardEvent, type MouseEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ContextMenu, type MenuEntry } from "../components/ContextMenu";
import { FileInfo } from "../components/FileInfo";
import { FileRow, KindIcon, NameInput } from "../components/FileRow";
import { FolderPicker } from "../components/FolderPicker";
import { Progress } from "../components/Progress";
import { Button, Notice } from "../components/ui";
import { bytes } from "../lib/format";
import { pickApp, pickFiles, pickFolder } from "../lib/pick";
import { type Selection, all, click, empty, move as moveSel } from "../lib/selection";
import { useStore } from "../lib/store";
import { ago } from "../lib/time";
import type { AppChoice, CloudEntry, CloudStatus, Conflict, Space, TrashEntry } from "../lib/types";

type SortKey = "name" | "size" | "modified";
const UNDO_MS = 6000;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const baseOf = (path: string) => path.split("/").filter(Boolean).pop() ?? path;
const parentOf = (rel: string) => (rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "");

function sorted(entries: CloudEntry[], key: SortKey, dir: 1 | -1): CloudEntry[] {
  const by = (a: CloudEntry, b: CloudEntry) =>
    key === "size" ? a.size - b.size : key === "modified" ? a.modified - b.modified : a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  return [...entries].sort((a, b) => Number(a.kind !== "dir") - Number(b.kind !== "dir") || dir * by(a, b));
}

function NotConnected({ status }: { status: CloudStatus | null }) {
  const { api, setScreen } = useStore();
  const [error, setError] = useState<string | null>(null);
  return (
    <div className="flex max-w-[560px] flex-col items-start gap-3 p-8">
      <h1 className="text-[15px] font-medium">Cloud not connected</h1>
      <Notice>
        Cloud is a folder on the home server, shared like the vault. Once the share is set up on the server, Connect opens it in Finder, which asks for the password the first time and mounts it at{" "}
        <span className="font-mono text-[11px]">{status?.path ?? "/Volumes/cloud"}</span>.
      </Notice>
      <div className="flex gap-2">
        <Button variant="primary" onClick={() => void api.cloudConnect().then(() => setError(null), (e) => setError(message(e)))}>
          Connect
        </Button>
        <Button variant="ghost" onClick={() => setScreen("settings")}>
          Settings
        </Button>
      </div>
      {error && <p className="text-[12px] text-urgent">{error}</p>}
    </div>
  );
}

/** Space on the disk of the folder on screen: the SD card inside Cloud is a disk of its own. */
function SpaceBar({ space }: { space: Space | null }) {
  if (!space) return null;
  const { total, free } = space;
  const used = total ? ((total - free) / total) * 100 : 0;
  return (
    <div className="flex w-48 flex-col gap-1">
      <span className="text-right font-mono text-[11px] text-text-3">{`${bytes(free)} free of ${bytes(total)}`}</span>
      <Progress value={used} label="Space used" />
    </div>
  );
}

function Trash({ onChange }: { onChange(): void }) {
  const { api } = useStore();
  const [items, setItems] = useState<TrashEntry[] | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => api.cloudTrashList().then(setItems, (e) => setError(message(e))), [api]);
  useEffect(() => void load(), [load]);
  const act = async (run: () => Promise<void>) => {
    try {
      await run();
      setError(null);
    } catch (e) {
      setError(message(e));
    }
    await load();
    onChange();
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 px-5 pb-5">
      <div className="flex h-8 items-center gap-2">
        <span className="text-[12px] text-text-3">Deleted items stay here until you empty the trash.</span>
        {confirm ? (
          <span className="ml-auto flex items-center gap-2 text-[12px]">
            <span className="text-text-2">Empty the trash? This can't be undone.</span>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              Keep
            </Button>
            <Button onClick={() => void act(() => api.cloudEmptyTrash()).then(() => setConfirm(false))}>Empty</Button>
          </span>
        ) : (
          <Button variant="ghost" className="ml-auto" disabled={!items?.length} onClick={() => setConfirm(true)}>
            Empty trash
          </Button>
        )}
      </div>
      {error && <p className="text-[12px] text-urgent">{error}</p>}
      {items?.length === 0 ? (
        <Notice>The trash is empty.</Notice>
      ) : (
        <div className="min-h-0 overflow-y-auto rounded-panel border border-line bg-panel">
          <table aria-label="Trash" className="w-full table-fixed border-collapse">
            <thead>
              <tr className="h-8 border-b border-line text-left text-[11px] text-text-3">
                <th className="px-3 font-normal">Name</th>
                <th className="px-3 font-normal">Was in</th>
                <th className="w-32 px-3 font-normal">Deleted</th>
                <th className="w-24 px-3" />
              </tr>
            </thead>
            <tbody>
              {items?.map((t) => (
                <tr key={t.id} className="h-8 border-b border-line text-[12px] last:border-b-0">
                  <td className="px-3">
                    <div className="flex min-w-0 items-center gap-2">
                      <KindIcon entry={t.entry} />
                      <span className={`truncate ${t.unfinished ? "text-text-3" : ""}`}>{t.unfinished ? `Unfinished upload: ${t.entry.name}` : t.entry.name}</span>
                    </div>
                  </td>
                  <td className="truncate px-3 font-mono text-[11px] text-text-3">{parentOf(t.origin) || "Cloud"}</td>
                  <td className="px-3 text-text-3">{t.deleted ? ago(t.deleted) : ""}</td>
                  <td className="px-3 text-right">
                    {!t.unfinished && (
                      <Button variant="ghost" aria-label={`Restore ${t.entry.name}`} onClick={() => void act(() => api.cloudRestore(t.id))}>
                        Restore
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

/** A small file manager over the Cloud share on the home server. */
export function Cloud() {
  const { api, cloudClipboard: clipboard, setCloudClipboard } = useStore();
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [space, setSpace] = useState<Space | null>(null);
  const [rel, setRel] = useState("");
  const [entries, setEntries] = useState<CloudEntry[]>([]);
  const [mode, setMode] = useState<"files" | "trash">("files");
  const [showHidden, setShowHidden] = useState(false);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "name", dir: 1 });
  const [sel, setSel] = useState<Selection>(empty);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [undo, setUndo] = useState<string[] | null>(null);
  const [clash, setClash] = useState<{ count: number; run(choice: Conflict): void } | null>(null);
  const [dropOn, setDropOn] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; on: "items" | "folder" } | null>(null);
  const [apps, setApps] = useState<AppChoice[]>([]);
  const [info, setInfo] = useState<string | null>(null);
  const [picking, setPicking] = useState<string[] | null>(null);
  const table = useRef<HTMLTableElement>(null);
  const mounted = status?.mounted ?? false;

  const refreshStatus = useCallback(() => api.cloudStatus().then(setStatus, () => setStatus(null)), [api]);
  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);
  // While not connected, look again every few seconds: Finder mounts the share on its own after Connect.
  useEffect(() => {
    if (mounted) return;
    const timer = window.setInterval(() => void refreshStatus(), 3000);
    return () => clearInterval(timer);
  }, [mounted, refreshStatus]);

  const refreshSpace = useCallback(() => {
    if (mounted) void api.cloudSpace(rel).then(setSpace, () => setSpace(status?.space ?? null));
  }, [api, rel, mounted, status?.space]);
  useEffect(() => refreshSpace(), [refreshSpace]);

  const load = useCallback(async () => {
    if (!mounted) return;
    try {
      setEntries(await api.cloudList(rel, showHidden));
      setError(null);
    } catch (e) {
      setError(message(e));
    }
  }, [api, rel, showHidden, mounted]);
  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => api.onTransfer((t) => t.state === "done" && void load().then(refreshSpace)), [api, load, refreshSpace]);

  useEffect(() => {
    if (!undo) return;
    const timer = window.setTimeout(() => setUndo(null), UNDO_MS);
    return () => clearTimeout(timer);
  }, [undo]);

  const act = async (run: () => Promise<unknown>) => {
    try {
      await run();
      setError(null);
    } catch (e) {
      setError(message(e));
    }
    await load();
  };
  const fail = (e: unknown) => setError(message(e));

  /** Runs `start` with the clash choice, asking first when some names are already taken. */
  const askOnClash = (count: number, start: (choice: Conflict) => void) => {
    if (!count) return start("keep_both");
    setClash({ count, run: (choice) => (setClash(null), start(choice)) });
  };

  const upload = useCallback(
    async (paths: string[], dest = rel) => {
      if (!paths.length) return;
      const there = dest === rel ? entries : await api.cloudList(dest, true).catch(() => []);
      const count = paths.filter((p) => there.some((e) => e.name === baseOf(p))).length;
      askOnClash(count, (choice) => void api.cloudUpload(paths, dest, choice).catch(fail));
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [api, entries, rel],
  );

  /** The folder under the pointer: a folder row or a breadcrumb, else the folder on screen. */
  const targetAt = (x: number, y: number) => document.elementFromPoint?.(x, y)?.closest("[data-drop-rel]")?.getAttribute("data-drop-rel") ?? rel;
  const onDropRef = useRef<(paths: string[], x: number, y: number) => void>(() => {});
  onDropRef.current = (paths, x, y) => {
    setDropOn(null);
    const target = targetAt(x, y);
    // Paths from the share itself (a row dragged out and back) are a move; anything else is an upload.
    const prefix = `${status?.path ?? "/Volumes/cloud"}/`;
    const inside = paths.filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length));
    const outside = paths.filter((p) => !p.startsWith(prefix));
    if (inside.length)
      void act(async () => {
        for (const src of inside) if (src !== target) await move(src, target);
      });
    if (outside.length) void upload(outside, target);
  };
  useEffect(() => api.onDrop((d) => mode === "files" && onDropRef.current(d.paths, d.x, d.y)), [api, mode]);
  const overRef = useRef<(p: { x: number; y: number } | null) => void>(() => {});
  overRef.current = (p) => {
    const t = p ? targetAt(p.x, p.y) : null;
    setDropOn(t && t !== rel ? t : null);
  };
  useEffect(() => api.onDragOver((p) => overRef.current(p)), [api]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return sorted(
      entries.filter((e) => !q || e.name.toLowerCase().includes(q)),
      sort.key,
      sort.dir,
    );
  }, [entries, query, sort]);
  const order = shown.map((e) => e.rel);
  const picked = shown.filter((e) => sel.items.includes(e.rel));
  const one = picked.length === 1 ? picked[0] : null;

  const go = (to: string) => {
    setRel(to);
    setSel(empty);
    setRenaming(null);
    setQuery("");
  };
  const open = (list: CloudEntry[]) => {
    if (list.length === 1 && list[0].kind === "dir") return go(list[0].rel);
    void act(async () => {
      for (const e of list) if (e.kind !== "dir") await api.cloudOpen(e.rel);
    });
  };
  const trash = (targets: string[]) =>
    act(async () => {
      for (const t of targets) await api.cloudTrash(t);
      const list = await api.cloudTrashList();
      const ids = targets.map((t) => list.find((x) => x.origin === t)?.id).filter((id): id is string => !!id);
      setSel(empty);
      if (ids.length) setUndo(ids);
    });
  const move = (src: string, destDir: string) => {
    if (src === destDir || parentOf(src) === destDir) return Promise.resolve();
    return api.cloudMove(src, destDir);
  };
  const paste = async (dest: string) => {
    if (!clipboard) return;
    const { mode: how, rels } = clipboard;
    if (how === "cut") {
      setCloudClipboard(null);
      return act(async () => {
        for (const r of rels) await move(r, dest);
      });
    }
    const there = dest === rel ? entries : await api.cloudList(dest, true).catch(() => []);
    const count = rels.filter((r) => there.some((e) => e.name === baseOf(r))).length;
    askOnClash(count, (choice) => void api.cloudCopy(rels, dest, choice).catch(fail));
  };
  const copyPath = (list: CloudEntry[]) => void navigator.clipboard?.writeText(list.map((e) => `${status?.path ?? "/Volumes/cloud"}/${e.rel}`).join("\n")).catch(fail);
  const saveTo = (list: CloudEntry[]) =>
    void pickFolder().then((dir) => {
      if (dir) api.cloudDownload(list.map((e) => e.rel), dir, "keep_both").catch(fail);
    });
  const newFolder = () => (setCreating(true), setSel(empty));

  const itemMenu = (): MenuEntry[] => {
    const list = picked;
    const zip = one && one.kind === "file" && one.name.toLowerCase().endsWith(".zip");
    const rels = list.map((e) => e.rel);
    const out: (MenuEntry | false | null)[] = [
      { label: "Open", shortcut: "⌘↓", onPick: () => open(list) },
      one?.kind === "file" && {
        label: "Open with",
        submenu: [
          ...apps.map((a) => ({ label: a.default ? `${a.name} (default)` : a.name, onPick: () => void api.cloudOpenWith(one.rel, a.path).catch(fail) })),
          { label: "Other…", onPick: () => void pickApp().then((app) => void (app && api.cloudOpenWith(one.rel, app).catch(fail))) },
        ],
      },
      one && { label: "Quick Look", shortcut: "Space", onPick: () => void api.cloudQuickLook(one.rel).catch(fail) },
      "separator",
      one && { label: "Rename", shortcut: "↩", onPick: () => setRenaming(one.rel) },
      { label: "Duplicate", shortcut: "⌘D", onPick: () => void api.cloudDuplicate(rels).catch(fail) },
      { label: "Copy", shortcut: "⌘C", onPick: () => setCloudClipboard({ mode: "copy", rels }) },
      { label: "Cut", shortcut: "⌘X", onPick: () => setCloudClipboard({ mode: "cut", rels }) },
      { label: "Move to…", onPick: () => setPicking(rels) },
      { label: "Save to…", onPick: () => saveTo(list) },
      "separator",
      { label: one ? `Compress "${one.name}"` : `Compress ${list.length} items`, onPick: () => void api.cloudCompress(rels).catch(fail) },
      zip && { label: "Extract here", onPick: () => void api.cloudExtract(one.rel).catch(fail) },
      "separator",
      { label: "Copy path", onPick: () => copyPath(list) },
      one && { label: "Get info", shortcut: "⌘I", onPick: () => setInfo(one.rel) },
      "separator",
      { label: "Move to trash", shortcut: "⌘⌫", onPick: () => void trash(rels) },
    ];
    return out.filter((x): x is MenuEntry => !!x);
  };
  const folderMenu = (): MenuEntry[] =>
    [
      { label: "New folder", shortcut: "⇧⌘N", onPick: newFolder },
      clipboard && { label: "Paste", shortcut: "⌘V", onPick: () => void paste(rel) },
      { label: "Upload…", onPick: () => void pickFiles().then((p) => upload(p)) },
      { label: showHidden ? "Hide hidden" : "Show hidden", onPick: () => setShowHidden(!showHidden) },
    ].filter((x): x is Exclude<typeof x, null> => !!x);

  const openMenu = (e: MouseEvent, on: "items" | "folder") => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, on });
  };
  const onRowMenu = (e: MouseEvent, entry: CloudEntry) => {
    e.stopPropagation();
    const list = sel.items.includes(entry.rel) ? shown.filter((x) => sel.items.includes(x.rel)) : [entry];
    if (!sel.items.includes(entry.rel)) setSel({ items: [entry.rel], anchor: entry.rel });
    setApps([]);
    if (list.length === 1 && list[0].kind === "file") void api.cloudAppsFor(entry.rel).then(setApps, () => {});
    openMenu(e, "items");
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (renaming || creating || menu || info || picking) return;
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    const rels = picked.map((x) => x.rel);
    if (key === "ArrowDown" && !e.metaKey) return handled(), setSel(moveSel(sel, order, 1, e.shiftKey));
    if (key === "ArrowUp" && !e.metaKey) return handled(), setSel(moveSel(sel, order, -1, e.shiftKey));
    if (key === "ArrowUp" && e.metaKey && rel) return handled(), go(parentOf(rel));
    if (key === "Escape") return handled(), setSel(empty);
    if (e.metaKey && key === "a") return handled(), setSel(all(order));
    if (e.metaKey && e.shiftKey && key === "n") return handled(), newFolder();
    if (e.metaKey && key === "v") return handled(), void paste(rel);
    if (!picked.length) return;
    if (e.metaKey && key === "c") return handled(), setCloudClipboard({ mode: "copy", rels });
    if (e.metaKey && key === "x") return handled(), setCloudClipboard({ mode: "cut", rels });
    if (e.metaKey && key === "d") return handled(), void api.cloudDuplicate(rels).catch(fail);
    if (e.metaKey && key === "Backspace") return handled(), void trash(rels);
    if ((key === "ArrowDown" && e.metaKey) || (e.metaKey && key === "o")) return handled(), open(picked);
    if (!one) return;
    if (e.metaKey && key === "i") return handled(), setInfo(one.rel);
    if (key === " ") return handled(), void api.cloudQuickLook(one.rel).catch(fail);
    if (key === "Enter" || key === "F2") return handled(), setRenaming(one.rel);
  };

  if (!mounted) return <NotConnected status={status} />;

  const crumbs = rel ? rel.split("/") : [];
  const header = (key: SortKey, label: string, className = "") => (
    <th aria-sort={sort.key === key ? (sort.dir === 1 ? "ascending" : "descending") : undefined} className={`px-3 font-normal ${className}`}>
      <button
        type="button"
        onClick={() => setSort((s) => ({ key, dir: s.key === key ? (s.dir === 1 ? -1 : 1) : 1 }))}
        className={`text-[11px] ${sort.key === key ? "text-text-2" : "text-text-3"} hover:text-text`}
      >
        {label}
      </button>
    </th>
  );
  const closeMenu = () => {
    setMenu(null);
    table.current?.focus();
  };

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <nav aria-label="Path" className="flex min-w-0 items-center gap-1 text-[13px]">
          {["", ...crumbs.map((_, i) => crumbs.slice(0, i + 1).join("/"))].map((to, i) => (
            <span key={to || "root"} className="flex min-w-0 items-center gap-1">
              {i > 0 && <span className="text-text-3">/</span>}
              <button
                type="button"
                aria-current={to === rel && mode === "files" ? "page" : undefined}
                onClick={() => (setMode("files"), go(to))}
                data-drop-rel={to}
                className={`truncate rounded-control px-1 ${to === rel ? "font-medium text-text" : "text-text-3 hover:text-text"} ${dropOn === to ? "bg-raised" : ""}`}
              >
                {i === 0 ? "Cloud" : baseOf(to)}
              </button>
            </span>
          ))}
        </nav>
        <div className="ml-auto">
          <SpaceBar space={space ?? status?.space ?? null} />
        </div>
      </header>
      <div className="flex h-10 shrink-0 items-center gap-2 px-5">
        <div className="flex h-7 items-center rounded-control border border-line bg-card p-0.5">
          {(["files", "trash"] as const).map((m) => (
            <button
              key={m}
              type="button"
              aria-pressed={mode === m}
              onClick={() => setMode(m)}
              className={`h-full rounded-[6px] px-2 text-[12px] ${mode === m ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
            >
              {m === "files" ? "Files" : "Trash"}
            </button>
          ))}
        </div>
        {mode === "files" && (
          <>
            <Button variant="primary" onClick={() => void pickFiles().then((p) => upload(p))}>
              Upload
            </Button>
            <Button onClick={newFolder}>New folder</Button>
            <Button variant="ghost" disabled={!picked.length} onClick={() => saveTo(picked)}>
              Save to…
            </Button>
            <input
              type="search"
              aria-label="Search this folder"
              placeholder="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="ml-auto h-7 w-44 rounded-control border border-line bg-card px-2 text-[12px] placeholder:text-text-3 focus:border-text-3 focus:outline-none"
            />
            <label className="flex items-center gap-2 text-[12px] text-text-3">
              <input type="checkbox" checked={showHidden} onChange={(e) => setShowHidden(e.target.checked)} className="accent-[#ededed]" />
              Show hidden
            </label>
          </>
        )}
      </div>
      {clash && (
        <div role="alertdialog" aria-label={clash.count === 1 ? "1 item already exists here" : `${clash.count} items already exist here`} className="mx-5 mb-2 flex items-center gap-2 rounded-panel border border-line-strong bg-panel px-3 py-2 text-[12px]">
          <span className="text-text-2">{clash.count === 1 ? "1 item already exists here." : `${clash.count} items already exist here.`}</span>
          <Button variant="ghost" className="ml-auto" onClick={() => setClash(null)}>
            Cancel
          </Button>
          <Button variant="ghost" onClick={() => clash.run("skip")}>
            Skip
          </Button>
          <Button onClick={() => clash.run("replace")}>Replace</Button>
          <Button variant="primary" onClick={() => clash.run("keep_both")}>
            Keep both
          </Button>
        </div>
      )}
      {error && <p className="px-5 pb-2 text-[12px] text-urgent">{error}</p>}
      {mode === "trash" ? (
        <Trash onChange={() => void load().then(refreshSpace)} />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col px-5 pb-5">
          <div className="flex max-h-full min-h-0 flex-1 flex-col overflow-y-auto rounded-panel border border-line bg-panel" onContextMenu={(e) => openMenu(e, "folder")}>
            <table ref={table} aria-label="Files" tabIndex={0} onKeyDown={onKeyDown} className="w-full table-fixed border-collapse focus:outline-none">
              <thead>
                <tr className="h-8 border-b border-line text-left">
                  {header("name", "Name")}
                  {header("size", "Size", "w-24 text-right")}
                  {header("modified", "Modified", "w-32")}
                </tr>
              </thead>
              <tbody>
                {creating && (
                  <tr className="h-8 border-b border-line">
                    <td className="px-3" colSpan={3}>
                      <NameInput
                        label="Folder name"
                        initial=""
                        onDone={(name) => {
                          setCreating(false);
                          table.current?.focus();
                          if (name) void act(() => api.cloudMkdir(rel, name));
                        }}
                      />
                    </td>
                  </tr>
                )}
                {shown.map((entry) => (
                  <FileRow
                    key={entry.rel}
                    entry={entry}
                    selected={sel.items.includes(entry.rel)}
                    renaming={renaming === entry.rel}
                    dropTarget={dropOn === entry.rel}
                    onSelect={(mods) => setSel(click(sel, entry.rel, order, mods))}
                    onOpen={() => open([entry])}
                    onMenu={(e) => onRowMenu(e, entry)}
                    onRename={(name) => {
                      setRenaming(null);
                      table.current?.focus();
                      if (name) void act(() => api.cloudRename(entry.rel, name));
                    }}
                    onDragStart={(e) => {
                      // A native drag, so Finder gets the files; dropped back here it comes in as a move.
                      e.preventDefault();
                      const rels = sel.items.includes(entry.rel) ? order.filter((r) => sel.items.includes(r)) : [entry.rel];
                      void api.startDrag(rels.map((r) => `${status?.path ?? "/Volumes/cloud"}/${r}`)).catch(fail);
                    }}
                  />
                ))}
              </tbody>
            </table>
            {shown.length === 0 && !creating && <p className="px-3 py-4 text-[12px] text-text-3">{query ? "No match in this folder." : "This folder is empty. Drop files here from Finder."}</p>}
            <div data-testid="cloud-empty-space" className="min-h-16 flex-1" onClick={() => setSel(empty)} />
          </div>
        </div>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.on === "items" ? itemMenu() : folderMenu()} onClose={closeMenu} />}
      {info && <FileInfo rel={info} onClose={() => (setInfo(null), table.current?.focus())} />}
      {picking && (
        <FolderPicker
          onClose={() => setPicking(null)}
          onPick={(dest) => {
            const srcs = picking;
            setPicking(null);
            void act(async () => {
              for (const s of srcs) await move(s, dest);
            });
          }}
        />
      )}
      {undo && (
        <div role="status" className="fade-in absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-3 rounded-panel border border-line-strong bg-panel px-3 py-2 text-[12px] shadow-menu">
          <span>Moved to trash.</span>
          <button
            type="button"
            onClick={() => {
              const ids = undo;
              setUndo(null);
              void act(async () => {
                for (const id of ids) await api.cloudRestore(id);
              });
            }}
            className="font-medium text-text hover:underline"
          >
            Undo
          </button>
        </div>
      )}
    </div>
  );
}
