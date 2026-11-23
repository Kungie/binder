import { type ReactNode, useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../lib/store";
import { GUIDES, NEEDS, type Need } from "../lib/guides";
import { MascotMark } from "./Mascot";
import { Button } from "./ui";

/** A screen's picture in a window frame. */
export function Shot({ src, title, className = "" }: { src: string; title: string; className?: string }) {
  return (
    <figure className={`overflow-hidden rounded-panel border border-line-strong bg-panel shadow-[0_24px_80px_rgba(0,0,0,0.55)] ${className}`}>
      <div className="flex h-7 items-center gap-1.5 border-b border-line px-3">
        {[0, 1, 2].map((i) => (
          <span key={i} className="size-2.5 rounded-full bg-raised" />
        ))}
        <span className="flex-1 text-center text-[11px] text-text-3">{title}</span>
        <span className="w-[42px]" />
      </div>
      <img src={src} alt={`The ${title} screen`} draggable={false} className="block aspect-[16/10] w-full bg-bg select-none" />
    </figure>
  );
}

/** Whether each thing a guide can need is there now, or null while it is being checked. */
export function useNeeds(needs: Need[], live: boolean): Partial<Record<Need, boolean | null>> {
  const { api } = useStore();
  const [state, setState] = useState<Partial<Record<Need, boolean | null>>>({});
  const key = needs.join(",");
  useEffect(() => {
    if (!live || !key) return;
    let on = true;
    const list = key.split(",") as Need[];
    setState(Object.fromEntries(list.map((n) => [n, null])));
    const set = (n: Need, v: boolean) => on && setState((s) => ({ ...s, [n]: v }));
    const checks = list.some((n) => n === "gh" || n === "tailscale") ? api.setupChecks() : null;
    for (const n of list) {
      const done = (p: Promise<boolean>) => void p.then((v) => set(n, v), () => set(n, false));
      if (n === "server") done(api.getConfig().then((c) => !!c.server));
      if (n === "printer") done(api.printers().then((p) => p.length > 0));
      if (n === "vault") done(api.vaultConnected());
      if (n === "cloud") done(api.cloudStatus().then((c) => c.mounted));
      if (n === "gh") done(checks!.then((c) => c.gh_signed_in));
      if (n === "tailscale") done(checks!.then((c) => c.tailscale));
    }
    return () => {
      on = false;
    };
  }, [api, key, live]);
  return state;
}

export function NeedRow({ label, ok }: { label: ReactNode; ok: boolean | null | undefined }) {
  return (
    <li className="flex items-center gap-3 py-2">
      <span aria-hidden className={`flex size-5 shrink-0 items-center justify-center rounded-full border ${ok ? "border-ok/40 text-ok" : "border-line-strong text-text-3"}`}>
        {ok ? (
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.6">
            <path d="M2 5.2l2 2 4-4.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        ) : (
          <span className="size-1 rounded-full bg-current" />
        )}
      </span>
      <span className="min-w-0 flex-1 text-[13px] text-text-2">{label}</span>
      <span className={`text-[12px] ${ok ? "text-ok" : "text-text-3"}`}>{ok === undefined || ok === null ? "Checking" : ok ? "Ready" : "Not yet"}</span>
    </li>
  );
}

const typing = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** A screen's guide over the whole window: the first time the screen opens, and again with `?` while it shows. */
export function ScreenGuide({ id }: { id: string }) {
  const { api, screen } = useStore();
  const [open, setOpen] = useState(false);
  const guide = GUIDES[id];
  const here = screen === id;
  useEffect(() => {
    if (!here) return setOpen(false);
    let live = true;
    void api.getConfig().then((c) => live && setOpen(!(c.tips_hidden ?? []).includes(id)), () => {});
    return () => {
      live = false;
    };
  }, [api, id, here]);
  useEffect(() => {
    if (!here) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "?" && !e.metaKey && !e.ctrlKey && !typing(e.target)) {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [here]);
  const needs = useNeeds(guide?.needs ?? [], open);
  const close = useCallback(async () => {
    setOpen(false);
    const c = await api.getConfig();
    if (!(c.tips_hidden ?? []).includes(id)) await api.setConfig({ ...c, tips_hidden: [...(c.tips_hidden ?? []), id] });
  }, [api, id]);
  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => (e.key === "Escape" || e.key === "Enter") && void close();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [open, close]);
  if (!guide || !open) return null;
  const missing = guide.needs.some((n) => needs[n] === false);
  return createPortal(
    <div role="dialog" aria-modal="true" aria-label={`About ${guide.title}`} className="guide-in fixed inset-0 z-50 flex overflow-y-auto bg-bg">
      <div className="m-auto grid w-full max-w-[1280px] grid-cols-1 items-center gap-10 px-10 py-12 lg:grid-cols-[minmax(300px,380px)_minmax(0,1fr)] lg:gap-14">
        <div className="flex flex-col gap-6">
          <div className="flex items-center gap-2 text-[12px] text-text-3">
            <MascotMark size={14} />
            <span>Guide</span>
          </div>
          <div className="flex flex-col gap-2">
            <h1 className="text-[28px] leading-8 font-medium tracking-[-0.02em] text-text">{guide.title}</h1>
            <p className="text-[15px] leading-6 text-text-2">{guide.lead}</p>
          </div>
          <ol className="flex flex-col gap-3">
            {guide.points.map((p, i) => (
              <li key={i} className="flex gap-3 text-[13px] leading-5 text-text-2">
                <span className="mt-px flex size-5 shrink-0 items-center justify-center rounded-full bg-raised font-mono text-[11px] text-text">{i + 1}</span>
                <span>{p}</span>
              </li>
            ))}
          </ol>
          {guide.needs.length > 0 && (
            <div className="flex flex-col gap-1">
              <span className="text-[11px] tracking-[0.04em] text-text-3 uppercase">What it needs</span>
              <ul className="flex flex-col divide-y divide-line border-y border-line">
                {guide.needs.map((n) => (
                  <NeedRow key={n} label={NEEDS[n]} ok={needs[n]} />
                ))}
              </ul>
            </div>
          )}
          <div className="flex items-center gap-2">
            <Button variant="primary" className="h-9 px-4 text-[13px]" autoFocus onClick={() => void close()}>
              {missing ? "Set it up" : "Got it"}
            </Button>
            <span className="ml-2 text-[12px] text-text-3">
              Press <kbd className="rounded-[4px] border border-line px-1 font-mono text-[11px]">?</kbd> to see this again
            </span>
          </div>
        </div>
        <Shot src={guide.shot} title={guide.title} />
      </div>
    </div>,
    document.body,
  );
}
