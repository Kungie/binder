import { useCallback, useEffect, useRef, useState } from "react";
import { Terminal } from "../components/Terminal";
import { Button } from "../components/ui";
import { useStore } from "../lib/store";

/** Below this content width the two terminals stack. */
const STACK_BELOW = 900;

/** Claude Code and Qwen3 4B side by side, each a tmux session on the home server. Stays mounted once opened. */
export function Agents({ shown }: { shown: boolean }) {
  const { api, setScreen } = useStore();
  const [target, setTarget] = useState<string | null | undefined>(undefined);
  const [narrow, setNarrow] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const focus = useRef<[() => void, () => void]>([() => {}, () => {}]);

  // Read again each time the screen shows, so a server set in Settings is picked up.
  useEffect(() => {
    if (!shown) return;
    void api.getConfig().then((c) => setTarget(c.server?.trim() ? c.server : null));
  }, [api, shown]);

  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const observer = new ResizeObserver(() => el.offsetWidth && setNarrow(el.offsetWidth < STACK_BELOW));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!shown) return;
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey || (e.key !== "1" && e.key !== "2")) return;
      e.preventDefault();
      focus.current[e.key === "1" ? 0 : 1]();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [shown]);

  const left = useCallback((f: () => void) => (focus.current[0] = f), []);
  const right = useCallback((f: () => void) => (focus.current[1] = f), []);

  return (
    <div ref={root} hidden={!shown} className="flex min-h-0 flex-1 flex-col">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Agents</h1>
        {target && <span className="font-mono text-[11px] text-text-3">{target}</span>}
      </header>
      {target === null && (
        <div className="flex items-center gap-2 px-5 text-[12px] text-text-2">
          Set the home server in Settings to use Agents.
          <Button variant="ghost" onClick={() => setScreen("settings")}>
            Settings
          </Button>
        </div>
      )}
      {target && (
        <div className={`flex min-h-0 flex-1 gap-3 px-5 pb-5 ${narrow ? "flex-col" : "flex-row"}`}>
          <Terminal agent="claude" name="Claude Code" session="binder-claude" registerFocus={left} />
          <Terminal agent="qwen" name="Qwen3 4B" session="binder-qwen" registerFocus={right} />
        </div>
      )}
    </div>
  );
}
