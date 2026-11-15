import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Notice, fieldClass } from "../components/ui";
import { useStore } from "../lib/store";
import type { Site, SiteCheck } from "../lib/types";

const EVERY = 30_000;
const PAGE_W = 1280;
const PAGE_H = 800;
const MB = 1024 * 1024;

const size = (bytes: number) => (bytes >= 1024 * MB ? `${(bytes / 1024 / MB).toFixed(1)} GB` : `${Math.round(bytes / MB)} MB`);
export const figures = (s: Site) => `CPU ${s.cpu.toFixed(1)}% · RAM ${size(s.mem_bytes)} · Disk ${size(s.disk_bytes)}`;

/** Why a site is down, or null when it is up: it answers below 500 and its containers run. */
export function downReason(s: Site, check: SiteCheck | undefined): string | null {
  if (!s.address) return "No address";
  if (check?.error) return check.error;
  if (check && check.status !== null && check.status >= 500) return `Answered ${check.status}`;
  const stopped = s.containers.find((c) => c.state !== "running");
  if (stopped) return `${stopped.name} is ${stopped.state}`;
  return null;
}

/** The websites on the home server, watched: a live preview, whether each answers, and what it uses. */
export function Sites() {
  const { api } = useStore();
  const [sites, setSites] = useState<Site[] | null>(null);
  const [checks, setChecks] = useState<Record<string, SiteCheck>>({});
  const [error, setError] = useState<string | null>(null);
  // Bumps on Refresh, so the previews are read again.
  const [round, setRound] = useState(0);

  const load = useCallback(async () => {
    try {
      const list = await api.sites();
      setSites(list);
      setError(null);
      for (const s of list) {
        if (!s.address) continue;
        const url = s.address;
        void api
          .siteCheck(url)
          .then((c) => setChecks((m) => ({ ...m, [url]: c })))
          .catch((e: unknown) => setChecks((m) => ({ ...m, [url]: { status: null, frameable: false, error: e instanceof Error ? e.message : String(e) } })));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);

  useEffect(() => {
    void load();
    const t = window.setInterval(() => void load(), EVERY);
    return () => window.clearInterval(t);
  }, [load]);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Sites</h1>
        {sites && <span className="font-mono text-[11px] text-text-3">{sites.length}</span>}
        <Button
          variant="ghost"
          className="ml-auto"
          onClick={() => {
            setRound((r) => r + 1);
            void load();
          }}
        >
          Refresh
        </Button>
      </header>
      <div className="px-5 pb-5">
        {error && <Notice>{error}</Notice>}
        {sites && sites.length === 0 && <Notice>No sites in ~/sites on the server.</Notice>}
        <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
          {sites?.map((s) => (
            <SiteCard key={s.name} site={s} check={s.address ? checks[s.address] : undefined} round={round} onSaved={load} />
          ))}
        </div>
      </div>
    </div>
  );
}

function SiteCard({ site, check, round, onSaved }: { site: Site; check: SiteCheck | undefined; round: number; onSaved(): void }) {
  const { api } = useStore();
  const reason = downReason(site, check);
  const known = !site.address || check !== undefined;
  return (
    <article aria-label={site.name} className="overflow-hidden rounded-panel border border-line bg-panel">
      <Preview site={site} check={check} round={round} />
      <div className="flex flex-col gap-1 border-t border-line px-3 py-2.5">
        <div className="flex items-center gap-2">
          {known && (
            <span
              role="img"
              aria-label={reason ? "Down" : "Up"}
              title={reason ?? "Answering"}
              className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${reason ? "bg-bad" : "bg-ok"}`}
            />
          )}
          <span className="truncate font-medium text-text">{site.name}</span>
          {site.address && (
            <a
              href={site.address}
              onClick={(e) => {
                e.preventDefault();
                void api.openLink(site.address!);
              }}
              className="ml-auto truncate font-mono text-[11px] text-text-3 hover:text-text"
            >
              {site.address.replace(/^https:\/\//, "")}
            </a>
          )}
        </div>
        {!site.address && <AddAddress name={site.name} onSaved={onSaved} />}
        <span className="font-mono text-[11px] text-text-3">{figures(site)}</span>
      </div>
    </article>
  );
}

/**
 * The site drawn at 1280 × 800 and scaled to the card, as a still page: fetched by the Mac, without scripts, nothing
 * moving, never clickable, and only once the card is near the screen. Live pages in every card made the screen stutter.
 */
function Preview({ site, check, round }: { site: Site; check: SiteCheck | undefined; round: number }) {
  const { api } = useStore();
  const box = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(typeof IntersectionObserver === "undefined");
  const [scale, setScale] = useState(0.25);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    if (typeof IntersectionObserver !== "undefined") {
      const io = new IntersectionObserver(([e]) => setVisible(e.isIntersecting), { rootMargin: "200px" });
      io.observe(el);
      return () => io.disconnect();
    }
  }, []);
  useEffect(() => {
    const el = box.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(([e]) => setScale(e.contentRect.width / PAGE_W));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const want = !!site.address && !!check && !check.error && visible;
  // The page read once, kept while the card stays; Refresh reads it again.
  const [snap, setSnap] = useState<{ round: number; html: string | false } | null>(null);
  const fresh = snap !== null && snap.round === round;
  useEffect(() => {
    if (!want || fresh) return;
    let live = true;
    api.siteSnapshot(site.address!).then(
      (html) => live && setSnap({ round, html }),
      () => live && setSnap({ round, html: false }),
    );
    return () => {
      live = false;
    };
  }, [api, want, fresh, round, site.address]);
  const html = snap && typeof snap.html === "string" ? snap.html : null;
  return (
    <div ref={box} className="relative aspect-[1280/800] overflow-hidden bg-card">
      {want && html !== null ? (
        <iframe
          title={`${site.name} preview`}
          srcDoc={html}
          sandbox=""
          tabIndex={-1}
          style={{ width: PAGE_W, height: PAGE_H, transform: `scale(${scale})`, transformOrigin: "0 0" }}
          className="pointer-events-none absolute left-0 top-0 border-0 bg-white"
        />
      ) : (
        <div className="flex h-full flex-col items-center justify-center gap-1">
          <span className="text-[28px] font-medium text-text-2">{site.name.slice(0, 1).toUpperCase()}</span>
          <span className="text-[11px] text-text-3">{site.name}</span>
        </div>
      )}
    </div>
  );
}

function AddAddress({ name, onSaved }: { name: string; onSaved(): void }) {
  const { api } = useStore();
  const [open, setOpen] = useState(false);
  const [value, setValue] = useState("");
  const [error, setError] = useState<string | null>(null);
  if (!open)
    return (
      <Button variant="ghost" className="-ml-2.5 self-start text-text-2" onClick={() => setOpen(true)}>
        Add address
      </Button>
    );
  const save = async () => {
    try {
      await api.setSiteAddress(name, value);
      setOpen(false);
      onSaved();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <div className="flex flex-col gap-1">
      <input
        autoFocus
        aria-label="Address"
        placeholder="example.com"
        value={value}
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") void save();
          if (e.key === "Escape") setOpen(false);
        }}
        className={`${fieldClass} font-mono`}
      />
      {error && <span className="text-[11px] text-text-2">{error}</span>}
    </div>
  );
}
