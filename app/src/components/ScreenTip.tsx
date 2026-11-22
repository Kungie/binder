import { useEffect, useState } from "react";
import { useStore } from "../lib/store";
import { TIPS } from "../lib/tips";
import { Button } from "./ui";

/** The screen's tip until it is closed; Settings can bring them all back. */
export function ScreenTip({ id, className = "mx-5 mb-3" }: { id: string; className?: string }) {
  const { api } = useStore();
  const [shown, setShown] = useState(false);
  const tip = TIPS[id];
  useEffect(() => {
    let live = true;
    void api.getConfig().then((c) => live && setShown(!(c.tips_hidden ?? []).includes(id)), () => {});
    return () => {
      live = false;
    };
  }, [api, id]);
  if (!tip || !shown) return null;
  const close = async () => {
    setShown(false);
    const c = await api.getConfig();
    await api.setConfig({ ...c, tips_hidden: [...new Set([...(c.tips_hidden ?? []), id])] });
  };
  return (
    <aside aria-label={`About ${tip.title}`} className={`${className} flex items-start gap-3 rounded-panel border border-line bg-panel px-3 py-2.5`}>
      <svg aria-hidden width="14" height="14" viewBox="0 0 14 14" className="mt-0.5 shrink-0 text-text-3" fill="none" stroke="currentColor" strokeWidth="1.3">
        <circle cx="7" cy="7" r="6" />
        <path d="M7 6.2v3.6M7 4.2v.1" strokeLinecap="round" />
      </svg>
      <p className="min-w-0 flex-1 text-[12px] leading-5 text-text-2">{tip.body}</p>
      <Button variant="ghost" onClick={() => void close()}>
        Got it
      </Button>
    </aside>
  );
}
