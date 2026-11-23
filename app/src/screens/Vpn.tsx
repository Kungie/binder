import { useCallback, useEffect, useRef, useState } from "react";
import { Globe } from "../components/Globe";
import { Panel } from "../components/Panel";
import { fieldClass } from "../components/ui";
import { REGIONS, type Region, regionOf } from "../lib/regions";
import { useStore } from "../lib/store";
import type { PingResult, TailnetPeer, TailnetStatus } from "../lib/types";
import { usePoll } from "../lib/usePoll";
import { ScreenTip } from "../components/ScreenTip";
import { Code, NeedsSetup } from "../components/NeedsSetup";

const SINCE_KEY = "binder.vpn.since";
const readSince = (): number | null => {
  try {
    const v = Number(localStorage.getItem(SINCE_KEY));
    return v > 0 ? v : null;
  } catch {
    return null;
  }
};
const writeSince = (v: number | null) => {
  try {
    if (v === null) localStorage.removeItem(SINCE_KEY);
    else localStorage.setItem(SINCE_KEY, String(v));
  } catch {
    // Private storage: the time on is only shown while the screen stays.
  }
};

const placeName = (r: Region) => (r.city === r.country ? r.city : `${r.city}, ${r.country}`);

const bytes = (n: number) => {
  if (n < 1e6) return `${(n / 1e3).toFixed(0)} kB`;
  if (n < 1e9) return `${(n / 1e6).toFixed(1)} MB`;
  return `${(n / 1e9).toFixed(2)} GB`;
};
const rate = (n: number) => (n < 1e6 ? `${(n / 1e3).toFixed(0)} kB/s` : `${(n / 1e6).toFixed(1)} MB/s`);
const duration = (s: number) => {
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return h < 48 ? `${h} h ${m % 60} min` : `${Math.floor(h / 24)} days`;
};

interface Sample {
  down: number;
  up: number;
}

/** The exit nodes on the tailnet as regions: switch the Mac's traffic through one, or off. */
export function Vpn() {
  const { api } = useStore();
  const [status, setStatus] = useState<TailnetStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [places, setPlaces] = useState<Record<string, string>>({});
  const [last, setLast] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null | undefined>(undefined);
  const [failed, setFailed] = useState<string | null>(null);
  const [ping, setPing] = useState<PingResult | null>(null);
  const [since, setSince] = useState<number | null>(readSince);
  const [speed, setSpeed] = useState<Sample[]>([]);
  const prev = useRef<{ at: number; id: string; rx: number; tx: number } | null>(null);

  useEffect(() => {
    void api.getConfig().then((c) => {
      setPlaces(c.vpn_regions ?? {});
      setLast(c.vpn_last ?? null);
    });
  }, [api]);

  const read = useCallback(async () => {
    try {
      const s = await api.tailnetStatus();
      setStatus(s);
      setError(null);
      const exit = s.peers.find((p) => p.exit_node);
      const now = Date.now() / 1000;
      // When it was seen turning on, kept across restarts of the screen.
      setSince((was) => {
        const next = exit ? (was ?? Math.floor(now)) : null;
        if (next !== was) writeSince(next);
        return next;
      });
      if (exit) {
        const p = prev.current;
        if (p && p.id === exit.id && now > p.at) {
          const dt = now - p.at;
          const sample = { down: Math.max(0, ((exit.rx_bytes ?? 0) - p.rx) / dt), up: Math.max(0, ((exit.tx_bytes ?? 0) - p.tx) / dt) };
          setSpeed((list) => [...list.slice(-29), sample]);
        }
        prev.current = { at: now, id: exit.id, rx: exit.rx_bytes ?? 0, tx: exit.tx_bytes ?? 0 };
      } else {
        prev.current = null;
        setSpeed([]);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);
  usePoll(read, 2_000);

  const nodes = (status?.running ? status.peers : []).filter((p) => p.exit_option);
  const inUse = nodes.find((p) => p.exit_node) ?? null;
  const on = !!inUse;
  const chosen = inUse ?? nodes.find((p) => p.ip === last) ?? nodes.find((p) => p.online) ?? nodes[0] ?? null;
  const home = regionOf(status?.peers.find((p) => p.this_mac)?.relay);
  const placeOf = (p: TailnetPeer): Region | undefined => regionOf(places[p.name] ?? p.relay);

  useEffect(() => {
    if (!inUse) return setPing(null);
    let live = true;
    const go = () => void api.tailnetPing(inUse.ip).then((r) => live && setPing(r), () => {});
    go();
    const t = setInterval(go, 10_000);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, [api, inUse?.ip]); // eslint-disable-line react-hooks/exhaustive-deps

  const set = async (exit: string | null) => {
    setFailed(null);
    setPending(exit);
    try {
      await api.vpnSet(exit);
      if (exit) setLast(exit);
      await read();
    } catch (e) {
      setFailed(e instanceof Error ? e.message : String(e));
    } finally {
      setPending(undefined);
    }
  };

  const place = async (name: string, code: string) => {
    const c = await api.getConfig();
    const next = { ...(c.vpn_regions ?? {}), [name]: code };
    await api.setConfig({ ...c, vpn_regions: next });
    setPlaces(next);
  };

  const busy = pending !== undefined;
  const word = busy ? (pending === null ? "Turning off" : "Connecting") : on ? "On" : "Off";
  const where = chosen ? placeOf(chosen) : undefined;
  const top = Math.max(1, ...speed.map((s) => Math.max(s.down, s.up)));
  const nowS = Math.floor(Date.now() / 1000);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">VPN</h1>
        <span className="text-[12px] text-text-3">Through your own exit nodes on Tailscale</span>
      </header>
      <ScreenTip id="vpn" />
      {error?.includes("isn't installed") ? (
        <div className="px-5 pb-3">
          <NeedsSetup
            title="Set up the VPN"
            steps={[
              "Install Tailscale on this Mac from tailscale.com/download and sign in.",
              <>
                Rent a small server where you want your traffic to come out, install Tailscale on it and run <Code>tailscale set --advertise-exit-node</Code>.
              </>,
              "Allow it as an exit node in Tailscale's admin console, under the machine's route settings. Each one becomes a region here.",
            ]}
            action={{ label: "Get Tailscale", run: () => void api.openLink("https://tailscale.com/download") }}
          />
        </div>
      ) : (
        error && <p className="px-5 text-[12px] text-text-3">{error}</p>
      )}
      {status && !status.running && <p className="px-5 text-[12px] text-text-3">Tailscale is off on this Mac.</p>}
      {status?.running && nodes.length === 0 && (
        <p className="max-w-[640px] px-5 text-[12px] leading-5 text-text-2">
          No exit nodes on your tailnet. On a server, run <span className="font-mono text-text">tailscale set --advertise-exit-node</span>, then allow it in Tailscale's admin console under the machine's
          route settings.
        </p>
      )}
      {nodes.length > 0 && (
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-6 px-5 pb-5">
          <div className="flex flex-col items-center justify-center gap-4 rounded-panel border border-line bg-panel p-6">
            <Globe home={home} nodes={nodes.map((n) => ({ name: n.name, region: placeOf(n) }))} active={chosen?.name ?? null} on={on} />
            <div className="flex flex-col items-center gap-3">
              <button
                type="button"
                role="switch"
                aria-label="VPN"
                aria-checked={on}
                disabled={busy || !chosen}
                onClick={() => void set(on ? null : chosen!.ip)}
                className={`relative flex h-16 w-16 items-center justify-center rounded-full border transition-colors duration-300 disabled:opacity-60 ${
                  on ? "border-ok bg-ok/15 text-ok" : "border-line-strong bg-card text-text-3 hover:text-text"
                }`}
              >
                <svg aria-hidden width="22" height="22" viewBox="0 0 22 22" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                  <path d="M11 3v8" />
                  <path d="M6.2 6.2a7 7 0 1 0 9.6 0" />
                </svg>
              </button>
              <div className="flex flex-col items-center gap-0.5">
                <span className={`text-[13px] font-medium ${on ? "text-ok" : "text-text"}`}>{word}</span>
                <span className="text-[12px] text-text-3">{where ? `${chosen?.name} · ${placeName(where)}` : chosen?.name}</span>
              </div>
              {failed && <p className="max-w-80 text-center text-[11px] text-bad">{failed}</p>}
            </div>
          </div>
          <div className="flex min-w-0 flex-col gap-3">
            <Panel title="Regions" action={<span className="font-mono text-[11px] text-text-3">{nodes.length}</span>}>
              <div role="radiogroup" aria-label="Regions" className="border-t border-line">
                {nodes.map((n) => {
                  const r = placeOf(n);
                  const checked = n.name === chosen?.name;
                  return (
                    <div key={n.id} className="flex items-center gap-3 border-b border-line px-3 py-2 last:border-b-0">
                      <button
                        type="button"
                        role="radio"
                        aria-checked={checked}
                        aria-label={`${n.name} ${r?.city ?? ""}`.trim()}
                        disabled={busy || !n.online}
                        onClick={() => {
                          if (on) {
                            if (!n.exit_node) void set(n.ip);
                          } else {
                            setLast(n.ip);
                            void api.getConfig().then((c) => api.setConfig({ ...c, vpn_last: n.ip }));
                          }
                        }}
                        className="flex min-w-0 flex-1 items-center gap-2 text-left text-[12px] disabled:opacity-50"
                      >
                        <span aria-hidden className={`flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-full border ${checked ? "border-text" : "border-line-strong"}`}>
                          {checked && <span className={`h-1.5 w-1.5 rounded-full ${on && n.exit_node ? "bg-ok" : "bg-text"}`} />}
                        </span>
                        <span className="text-text">{n.name}</span>
                        <span className="truncate text-text-3">{r ? placeName(r) : "Unknown place"}</span>
                        {!n.online && <span className="text-[11px] text-bad">Offline</span>}
                      </button>
                      <select aria-label={`Place of ${n.name}`} className={`${fieldClass} !w-36 shrink-0`} value={places[n.name] ?? n.relay ?? ""} onChange={(e) => void place(n.name, e.target.value)}>
                        {!regionOf(places[n.name] ?? n.relay) && <option value="">Pick a place</option>}
                        {REGIONS.map((x) => (
                          <option key={x.code} value={x.code}>
                            {x.city}
                          </option>
                        ))}
                      </select>
                    </div>
                  );
                })}
              </div>
            </Panel>
            <Panel title="Statistics" label="Statistics" className="flex flex-1 flex-col">
              <div className="grid grid-cols-2 gap-px border-t border-line bg-line">
                <Stat label="Ping" value={on ? (ping ? `${Math.round(ping.ms)} ms` : "…") : "Off"} note={on && ping ? (ping.via === "direct" ? "Direct" : `Through ${ping.via}`) : undefined} tone={ping?.via === "direct" ? "ok" : undefined} />
                <Stat label="On for" value={on && since ? duration(nowS - since) : on ? "On" : "Off"} />
                <Stat label="Download" value={on && speed.length ? rate(speed[speed.length - 1].down) : "0 kB/s"} />
                <Stat label="Upload" value={on && speed.length ? rate(speed[speed.length - 1].up) : "0 kB/s"} />
                <Stat label="Downloaded" value={bytes(inUse?.rx_bytes ?? chosen?.rx_bytes ?? 0)} stat="down" note="Since Tailscale started" />
                <Stat label="Uploaded" value={bytes(inUse?.tx_bytes ?? chosen?.tx_bytes ?? 0)} stat="up" />
              </div>
              <div className="flex flex-1 flex-col border-t border-line px-3 py-3">
                <span className="text-[11px] text-text-3">Speed, last minute</span>
                <div role="img" aria-label="Speed over the last minute" className="mt-2 flex min-h-12 flex-1 items-end gap-[3px]">
                  {Array.from({ length: 30 }, (_, i) => speed[speed.length - 30 + i]).map((s, i) => (
                    <span key={i} className="flex h-full flex-1 flex-col justify-end gap-px">
                      <span className="rounded-[1px] bg-ok/70" style={{ height: s ? `${Math.max(2, (s.down / top) * 100)}%` : "2%" }} />
                    </span>
                  ))}
                </div>
              </div>
            </Panel>
          </div>
        </div>
      )}
    </div>
  );
}

function Stat({ label, value, note, tone, stat }: { label: string; value: string; note?: string; tone?: "ok"; stat?: string }) {
  return (
    <div className="flex flex-col gap-0.5 bg-panel px-3 py-2.5">
      <span className="text-[11px] text-text-3">{label}</span>
      <span data-stat={stat} className="font-mono text-[15px] text-text">
        {value}
      </span>
      {note && <span className={`text-[11px] ${tone === "ok" ? "text-ok" : "text-text-3"}`}>{note}</span>}
    </div>
  );
}
