import { type DragEvent, type KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FileRow, KindIcon, NameInput } from "../components/FileRow";
import { Progress } from "../components/Progress";
import { Button, Notice } from "../components/ui";
import { bytes } from "../lib/format";
import { pickFiles, pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { ago } from "../lib/time";
import type { CloudEntry, CloudStatus, Conflict, TrashEntry } from "../lib/types";

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

function SpaceBar({ status }: { status: CloudStatus }) {
  if (!status.space) return null;
  const { total, free } = status.space;
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
                      <span className="truncate">{t.entry.name}</span>
                    </div>
                  </td>
                  <td className="truncate px-3 font-mono text-[11px] text-text-3">{parentOf(t.origin) || "Cloud"}</td>
                  <td className="px-3 text-text-3">{t.deleted ? ago(t.deleted) : ""}</td>
                  <td className="px-3 text-right">
                    <Button variant="ghost" aria-label={`Restore ${t.entry.name}`} onClick={() => void act(() => api.cloudRestore(t.id))}>
                      Restore
                    </Button>
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
  const { api } = useStore();
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [rel, setRel] = useState("");
  const [entries, setEntries] = useState<CloudEntry[]>([]);
  const [mode, setMode] = useState<"files" | "trash">("files");
  const [showHidden, setShowHidden] = useState(false);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "name", dir: 1 });
  const [selected, setSelected] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [undo, setUndo] = useState<{ name: string; id: string } | null>(null);
  const [clash, setClash] = useState<{ paths: string[]; count: number } | null>(null);
  const [dropOn, setDropOn] = useState<string | null>(null);
  const dragging = useRef<string | null>(null);
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

  useEffect(() => api.onTransfer((t) => t.state === "done" && void load().then(refreshStatus)), [api, load, refreshStatus]);

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

  const upload = useCallback(
    (paths: string[], choice?: Conflict) => {
      if (!paths.length) return;
      const count = paths.filter((p) => entries.some((e) => e.name === baseOf(p))).length;
      if (count && !choice) return setClash({ paths, count });
      setClash(null);
      void api.cloudUpload(paths, rel, choice ?? "keep_both").catch((e) => setError(message(e)));
    },
    [api, entries, rel],
  );
  const latestUpload = useRef(upload);
  latestUpload.current = upload;
  // Files dropped on the window from Finder land in the folder on screen.
  useEffect(() => api.onDrop((paths) => mode === "files" && latestUpload.current(paths)), [api, mode]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return sorted(
      entries.filter((e) => !q || e.name.toLowerCase().includes(q)),
      sort.key,
      sort.dir,
    );
  }, [entries, query, sort]);

  const go = (to: string) => {
    setRel(to);
    setSelected(null);
    setRenaming(null);
    setQuery("");
  };
  const open = (e: CloudEntry) => (e.kind === "dir" ? go(e.rel) : void act(() => api.cloudOpen(e.rel)));
  const trash = (target: string) =>
    act(async () => {
      await api.cloudTrash(target);
      const item = (await api.cloudTrashList()).find((t) => t.origin === target);
      setSelected(null);
      if (item) setUndo({ name: item.entry.name, id: item.id });
    });
  const move = (src: string, destDir: string) => {
    if (src === destDir || parentOf(src) === destDir) return;
    void act(() => api.cloudMove(src, destDir));
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (renaming || creating) return;
    const index = shown.findIndex((x) => x.rel === selected);
    const current = shown[index];
    const pick = (i: number) => shown[i] && setSelected(shown[i].rel);
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (e.key === "ArrowDown" && !e.metaKey) return handled(), pick(Math.min(shown.length - 1, index + 1));
    if (e.key === "ArrowUp" && !e.metaKey) return handled(), pick(Math.max(0, index - 1));
    if (e.key === "ArrowUp" && e.metaKey && rel) return handled(), go(parentOf(rel));
    if (!current) return;
    if (e.key === "Enter" || e.key === "F2") return handled(), setRenaming(current.rel);
    if ((e.key === "ArrowDown" && e.metaKey) || (e.key === "o" && e.metaKey)) return handled(), open(current);
    if (e.key === "Backspace" && e.metaKey) return handled(), void trash(current.rel);
  };

  const dropZone = (target: string) => ({
    onDragOver: (e: DragEvent) => {
      if (!dragging.current || dragging.current === target) return;
      e.preventDefault();
      setDropOn(target);
    },
    onDragLeave: () => setDropOn((d) => (d === target ? null : d)),
    onDrop: (e: DragEvent) => {
      e.preventDefault();
      const src = dragging.current ?? e.dataTransfer.getData("text/plain");
      dragging.current = null;
      setDropOn(null);
      if (src) move(src, target);
    },
  });

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

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <nav aria-label="Path" className="flex min-w-0 items-center gap-1 text-[13px]">
          {["", ...crumbs.map((_, i) => crumbs.slice(0, i + 1).join("/"))].map((to, i) => (
            <span key={to || "root"} className="flex min-w-0 items-center gap-1">
              {i > 0 && <span className="text-text-3">/</span>}
              <button
                type="button"
                aria-current={to === rel && mode === "files" ? "page" : undefined}
                onClick={() => (setMode("files"), go(to))}
                {...dropZone(to)}
                className={`truncate rounded-control px-1 ${to === rel ? "font-medium text-text" : "text-text-3 hover:text-text"} ${dropOn === to ? "bg-raised" : ""}`}
              >
                {i === 0 ? "Cloud" : baseOf(to)}
              </button>
            </span>
          ))}
        </nav>
        <div className="ml-auto">
          <SpaceBar status={status!} />
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
            <Button onClick={() => (setCreating(true), setSelected(null))}>New folder</Button>
            <Button
              variant="ghost"
              disabled={!selected}
              onClick={() => {
                if (!selected) return;
                void pickFolder().then((dir) => {
                  if (dir) api.cloudDownload([selected], dir, "keep_both").catch((e) => setError(message(e)));
                });
              }}
            >
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
          <Button variant="ghost" onClick={() => upload(clash.paths, "skip")}>
            Skip
          </Button>
          <Button onClick={() => upload(clash.paths, "replace")}>Replace</Button>
          <Button variant="primary" onClick={() => upload(clash.paths, "keep_both")}>
            Keep both
          </Button>
        </div>
      )}
      {error && <p className="px-5 pb-2 text-[12px] text-urgent">{error}</p>}
      {mode === "trash" ? (
        <Trash onChange={() => void load().then(refreshStatus)} />
      ) : (
        <div className="min-h-0 flex-1 px-5 pb-5">
          <div className="max-h-full overflow-y-auto rounded-panel border border-line bg-panel">
            <table aria-label="Files" tabIndex={0} onKeyDown={onKeyDown} className="w-full table-fixed border-collapse focus:outline-none">
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
                    selected={selected === entry.rel}
                    renaming={renaming === entry.rel}
                    dropTarget={dropOn === entry.rel}
                    onSelect={() => setSelected(entry.rel)}
                    onOpen={() => open(entry)}
                    onRename={(name) => {
                      setRenaming(null);
                      if (name) void act(() => api.cloudRename(entry.rel, name));
                    }}
                    onDragStart={(e) => {
                      dragging.current = entry.rel;
                      e.dataTransfer.setData("text/plain", entry.rel);
                    }}
                    {...dropZone(entry.rel)}
                  />
                ))}
              </tbody>
            </table>
            {shown.length === 0 && !creating && <p className="px-3 py-4 text-[12px] text-text-3">{query ? "No match in this folder." : "This folder is empty. Drop files here from Finder."}</p>}
          </div>
        </div>
      )}
      {undo && (
        <div role="status" className="fade-in absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-3 rounded-panel border border-line-strong bg-panel px-3 py-2 text-[12px] shadow-menu">
          <span>Moved to trash.</span>
          <button
            type="button"
            onClick={() => {
              const { id } = undo;
              setUndo(null);
              void act(() => api.cloudRestore(id));
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
