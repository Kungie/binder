import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";
import { useStore } from "../lib/store";
import type { Agent } from "../lib/types";
import { Dot } from "./Metric";
import { Panel } from "./Panel";
import { Button } from "./ui";
import { XtermView } from "./XtermView";

/** ⌘ keys that still reach Binder while a terminal has focus. */
const PASS = new Set(["k", "1", "2"]);

/** One agent's terminal: its tmux session on the server, over ssh. Connects once, on mount. */
export function Terminal({ agent, name, session, registerFocus }: { agent: Agent; name: string; session: string; registerFocus(focus: () => void): void }) {
  const { api } = useStore();
  const [connected, setConnected] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const size = useRef({ cols: 80, rows: 24 });
  // Output that arrives before xterm exists waits here.
  const pending = useRef<Uint8Array[]>([]);
  const write = useRef<(bytes: Uint8Array) => void>((b) => pending.current.push(b));
  const focus = useRef<() => void>(() => {});
  const opened = useRef(false);

  const sink = useCallback((bytes: Uint8Array) => write.current(bytes), []);

  const connect = useCallback(
    (run: typeof api.termOpen) => {
      setMessage(null);
      run(agent, size.current.cols, size.current.rows, sink).then(
        () => setConnected(true),
        (e: unknown) => {
          setConnected(false);
          setMessage(e instanceof Error ? e.message : String(e));
        },
      );
    },
    [agent, sink],
  );

  useEffect(
    () =>
      api.onTermExit((a, m) => {
        if (a !== agent) return;
        setConnected(false);
        setMessage(m);
      }),
    [api, agent],
  );

  useEffect(() => {
    if (opened.current) return;
    opened.current = true;
    connect(api.termOpen);
  }, [api, connect]);

  const register = useCallback(
    (w: (bytes: Uint8Array) => void, f: () => void) => {
      write.current = w;
      focus.current = f;
      for (const b of pending.current.splice(0)) w(b);
      registerFocus(() => focus.current());
    },
    [registerFocus],
  );

  const onData = useCallback((data: string) => void api.termWrite(agent, data).catch(() => {}), [api, agent]);
  const onResize = useCallback(
    (cols: number, rows: number) => {
      size.current = { cols, rows };
      void api.termResize(agent, cols, rows).catch(() => {});
    },
    [api, agent],
  );

  const onCopy = useCallback((text: string) => void api.copyText(text).catch(() => {}), [api]);

  // Every key goes to the terminal; only ⌘K and ⌘1/⌘2 travel on to Binder.
  const keep = (e: KeyboardEvent) => {
    if (!(e.metaKey && PASS.has(e.key.toLowerCase()))) e.stopPropagation();
  };

  return (
    <Panel
      label={name}
      className="flex min-h-0 min-w-0 flex-1 basis-0 flex-col"
      title={
        <span className="flex items-center gap-2">
          {name}
          <Dot on={connected} label={connected ? "Connected" : "Not connected"} />
          <span className="font-mono text-[11px] font-normal text-text-3">{session}</span>
        </span>
      }
      action={
        !connected &&
        message !== null && (
          <Button variant="ghost" onClick={() => connect(api.termReconnect)}>
            Reconnect
          </Button>
        )
      }
    >
      <div className="flex min-h-0 flex-1 flex-col border-t border-line" onKeyDown={keep} onMouseDown={() => focus.current()}>
        {message && <p className="px-3 pt-2 font-mono text-[11px] text-bad">{message}</p>}
        <div className="min-h-0 flex-1 p-2">
          <XtermView onData={onData} onResize={onResize} onCopy={onCopy} register={register} />
        </div>
      </div>
    </Panel>
  );
}
