import { useCallback, useEffect, useState } from "react";
import { pickFiles } from "../lib/pick";
import { useStore } from "../lib/store";
import type { PingResult, PingSample, TailnetPeer, TailnetStatus } from "../lib/types";
import { usePoll } from "../lib/usePoll";
import { Dot } from "./Metric";
import { Panel } from "./Panel";
import { Terminal } from "./Terminal";
import { Button, fieldClass } from "./ui";

const ago = (at: number, now: number) => {
  const m = Math.max(0, Math.round((now - at) / 60));
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h} h ago`;
  return `${Math.round(h / 24)} days ago`;
};

/** A key that expired, or one that expires within a week. */
function KeyNote({ peer, now }: { peer: TailnetPeer; now: number }) {
  if (peer.expired) return <span className="text-bad">Key expired</span>;
  if (peer.key_expiry === null || peer.key_expiry - now > 7 * 86_400) return null;
  const days = Math.max(0, Math.floor((peer.key_expiry - now) / 86_400));
  return <span className="text-st-progress">{`Key expires in ${days} day${days === 1 ? "" : "s"}`}</span>;
}

/** The last day's pings as bars, taller is slower, a gap where one went unanswered. */
function Spark({ samples }: { samples: PingSample[] }) {
  if (samples.length < 2) return null;
  const top = Math.max(1, ...samples.map((s) => s.ms ?? 0));
  const tail = samples.slice(-48);
  return (
    <span role="img" aria-label="Ping, last 24 hours" title={`${Math.round(Math.min(...tail.filter((s) => s.ms !== null).map((s) => s.ms!)))} to ${Math.round(top)} ms`} className="flex h-4 items-end gap-px">
      {tail.map((s, i) => (
        <span key={i} className={`w-[2px] rounded-[1px] ${s.ms === null ? "h-full bg-bad/50" : s.via === "direct" ? "bg-ok/70" : "bg-st-progress/70"}`} style={s.ms === null ? undefined : { height: `${Math.max(12, Math.round((s.ms / top) * 100))}%` }} />
      ))}
    </span>
  );
}

/** The tailnet from the Mac's Tailscale app: every device, its route and ping, Taildrop and an SSH shell. */
export function Tailnet() {
  const { api } = useStore();
  const [status, setStatus] = useState<TailnetStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pings, setPings] = useState<Record<string, PingResult | null>>({});
  const [history, setHistory] = useState<Record<string, PingSample[]>>({});
  const [note, setNote] = useState<string | null>(null);
  const [user, setUser] = useState("");
  const [asking, setAsking] = useState<{ peer: TailnetPeer; target: string } | null>(null);
  const [shell, setShell] = useState<{ name: string; target: string } | null>(null);

  const load = useCallback(async () => {
    try {
      const s = await api.tailnetStatus();
      setStatus(s);
      setError(null);
      setHistory(await api.tailnetHistory().catch(() => ({})));
      const live = s.peers.filter((p) => p.online && !p.this_mac);
      const got = await Promise.all(live.map(async (p) => [p.id, await api.tailnetPing(p.ip).catch(() => null)] as const));
      setPings(Object.fromEntries(got));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);
  usePoll(load, 60_000);

  useEffect(() => {
    void api.getConfig().then((c) => setUser(c.server?.includes("@") ? c.server.split("@")[0] : ""), () => {});
  }, [api]);

  const send = async (p: TailnetPeer) => {
    const paths = await pickFiles();
    if (paths.length === 0) return;
    setNote(`Sending to ${p.name}…`);
    try {
      await api.taildrop(p.dns || p.ip, paths);
      const what = paths.length === 1 ? paths[0].split("/").pop() : `${paths.length} files`;
      setNote(`Sent ${what} to ${p.name}.`);
    } catch (e) {
      setNote(e instanceof Error ? e.message : String(e));
    }
  };

  const closeShell = () => {
    if (shell) void api.termClose(`ssh-${shell.target.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`);
    setShell(null);
  };

  const now = Math.floor(Date.now() / 1000);
  const peers = status?.peers ?? [];
  const up = peers.filter((p) => p.online).length;

  return (
    <div className="flex flex-col gap-3">
      <Panel title="Tailscale" label="Tailscale" action={status && <span className="font-mono text-[11px] text-text-3">{status.running ? `${up} of ${peers.length} online` : "Stopped"}</span>}>
        {error && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">{error}</p>}
        {status && !status.running && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">Tailscale is off on this Mac.</p>}
        {status?.running && (
          <table aria-label="Tailnet" className="w-full table-fixed border-t border-line text-[12px]">
            <thead>
              <tr className="text-left text-[11px] text-text-3">
                <th className="py-2 pl-3 font-normal">Name</th>
                <th className="w-[80px] py-2 font-normal">System</th>
                <th className="w-[120px] py-2 font-normal">Address</th>
                <th className="py-2 font-normal">Status</th>
                <th className="w-[150px] py-2 pr-3 font-normal" aria-label="Actions" />
              </tr>
            </thead>
            <tbody>
              {peers.map((p) => {
                const ping = pings[p.id];
                return (
                  <tr key={p.id} className="h-9 border-t border-line">
                    <td className="truncate pl-3 text-text">
                      {p.name}
                      {p.this_mac && <span className="ml-2 text-text-3">This Mac</span>}
                    </td>
                    <td className="text-text-2">{p.os}</td>
                    <td className="truncate font-mono text-[11px] text-text-2">{p.ip}</td>
                    <td>
                      <span className="flex items-center gap-2 text-text-2">
                        <Dot on={p.online} />
                        {p.online ? "Online" : "Offline"}
                        {!p.online && p.last_seen !== null && <span className="text-text-3">{ago(p.last_seen, now)}</span>}
                        {ping && <span className={`font-mono text-[11px] ${ping.via === "direct" ? "text-ok" : "text-st-progress"}`}>{`${Math.round(ping.ms)} ms · ${ping.via}`}</span>}
                        <Spark samples={history[p.id] ?? []} />
                        <KeyNote peer={p} now={now} />
                      </span>
                    </td>
                    <td className="pr-3">
                      {!p.this_mac && (
                        <span className="flex justify-end gap-1">
                          {p.taildrop && (
                            <Button variant="ghost" onClick={() => void send(p)}>
                              Send file
                            </Button>
                          )}
                          {p.online && (
                            <Button variant="ghost" onClick={() => setAsking({ peer: p, target: `${user ? `${user}@` : ""}${p.dns || p.ip}` })}>
                              SSH
                            </Button>
                          )}
                        </span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
        {note && <p className="border-t border-line px-3 py-2 text-[12px] text-text-2">{note}</p>}
        {asking && (
          <form
            className="flex items-center gap-2 border-t border-line px-3 py-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (shell) closeShell();
              setShell({ name: asking.peer.name, target: asking.target.trim() });
              setAsking(null);
            }}
          >
            <span className="text-[12px] text-text-2">SSH to {asking.peer.name} as</span>
            <input aria-label="SSH target" autoFocus className={`${fieldClass} w-72 font-mono`} value={asking.target} onChange={(e) => setAsking({ ...asking, target: e.target.value })} />
            <Button type="submit">Connect</Button>
            <Button variant="ghost" onClick={() => setAsking(null)}>
              Cancel
            </Button>
          </form>
        )}
      </Panel>
      {shell && (
        <div className="flex h-[420px] flex-col">
          <Terminal
            key={shell.target}
            pane={{ kind: "ssh", target: shell.target }}
            name={`SSH · ${shell.name}`}
            registerFocus={() => {}}
            actions={
              <Button variant="ghost" onClick={closeShell}>
                Close
              </Button>
            }
          />
        </div>
      )}
    </div>
  );
}
