import { useEffect, useRef, useState } from "react";
import { useStore } from "../lib/store";
import type { Config } from "../lib/types";
import { Button, fieldClass } from "./ui";

/** The server in use, by its own name; a click opens the list of servers to switch to or add one. */
export function ServerSwitcher({ target, hostname, onSwitched }: { target: string; hostname?: string; onSwitched(): void }) {
  const { api } = useStore();
  const [open, setOpen] = useState(false);
  const [adding, setAdding] = useState(false);
  const [typed, setTyped] = useState("");
  const [cfg, setCfg] = useState<Config | null>(null);
  const box = useRef<HTMLDivElement>(null);

  // Remember this server and its name, so the list can show it by name later.
  useEffect(() => {
    if (!hostname) return;
    void api.getConfig().then(async (c) => {
      const servers = c.servers?.includes(target) ? c.servers : [...(c.servers ?? []), target];
      if (c.server_names?.[target] === hostname && servers === c.servers) return setCfg(c);
      const next = { ...c, servers, server_names: { ...(c.server_names ?? {}), [target]: hostname } };
      await api.setConfig(next);
      setCfg(next);
    });
  }, [api, target, hostname]);

  useEffect(() => {
    if (!open) return;
    void api.getConfig().then(setCfg);
    const away = (e: MouseEvent) => !box.current?.contains(e.target as Node) && (setOpen(false), setAdding(false));
    const esc = (e: KeyboardEvent) => e.key === "Escape" && (setOpen(false), setAdding(false));
    document.addEventListener("mousedown", away);
    window.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", away);
      window.removeEventListener("keydown", esc);
    };
  }, [api, open]);

  const use = async (t: string) => {
    const c = await api.getConfig();
    const servers = c.servers?.includes(t) ? c.servers : [...(c.servers ?? []), t];
    await api.setConfig({ ...c, server: t, servers });
    setOpen(false);
    setAdding(false);
    setTyped("");
    onSwitched();
  };

  const names = cfg?.server_names ?? {};
  const list = cfg?.servers?.length ? cfg.servers : [target];
  const name = hostname || names[target] || target;

  return (
    <div ref={box} className="relative">
      <button
        type="button"
        aria-label={`Server: ${name}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="flex h-7 items-center gap-1.5 rounded-control px-2 text-[12px] text-text hover:bg-raised"
      >
        {name}
        <svg aria-hidden width="10" height="10" viewBox="0 0 10 10" className={`text-text-3 transition-transform duration-[120ms] ${open ? "rotate-180" : ""}`}>
          <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <div role="menu" aria-label="Servers" className="absolute left-0 top-8 z-40 flex w-72 flex-col rounded-panel border border-line bg-panel p-1 shadow-menu">
          {list.map((t) => (
            <button
              key={t}
              type="button"
              role="menuitemradio"
              aria-checked={t === target}
              onClick={() => void use(t)}
              className="flex items-center gap-2 rounded-control px-2 py-1.5 text-left text-[12px] hover:bg-raised"
            >
              <span aria-hidden className={`h-1.5 w-1.5 shrink-0 rounded-full ${t === target ? "bg-ok" : "border border-line-strong"}`} />
              <span className="text-text">{names[t] || t}</span>
              <span className="ml-auto truncate font-mono text-[11px] text-text-3">{t}</span>
            </button>
          ))}
          <div className="mt-1 border-t border-line pt-1">
            {adding ? (
              <form
                className="flex gap-1 p-1"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (typed.trim()) void use(typed.trim());
                }}
              >
                <input aria-label="SSH target" autoFocus className={`${fieldClass} min-w-0 flex-1 font-mono`} placeholder="user@host" value={typed} onChange={(e) => setTyped(e.target.value)} />
                <Button type="submit">Add</Button>
              </form>
            ) : (
              <button type="button" onClick={() => setAdding(true)} className="w-full rounded-control px-2 py-1.5 text-left text-[12px] text-text-2 hover:bg-raised">
                Add server
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
