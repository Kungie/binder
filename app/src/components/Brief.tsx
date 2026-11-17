import { useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { Brief as BriefData, BriefItem } from "../lib/types";
import { Moon } from "./Moon";
import { Panel } from "./Panel";
import { useTaskFlow } from "./TaskBoard";
import { Button } from "./ui";

const clock = (t: number) => {
  const d = new Date(t * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

const PROBLEM = { stopped: "stopped at", restarting: "kept restarting from", unhealthy: "was unhealthy from" } as const;

/** One line's words, as the spec has them. */
export function briefText(i: BriefItem): string {
  switch (i.kind) {
    case "run":
      return `${i.key} ${i.ok ? "done" : "failed"}`;
    case "container":
      return `${i.name} ${PROBLEM[i.problem]} ${clock(i.at)}${i.back_at ? `, running again at ${clock(i.back_at)}` : ""}`;
    case "server_restart":
      return `The server restarted at ${clock(i.at)}`;
    case "print":
      return `${i.job} ${i.ok ? "is done" : "failed"}`;
    case "issue":
      return `${i.key} from GitHub: ${i.title}`;
    case "site":
      return i.back_at ? `${i.name} was down from ${clock(i.at)}, up again at ${clock(i.back_at)}` : `${i.name} went down at ${clock(i.at)}`;
    case "disk":
      return `${i.name} is ${i.percent}% full`;
  }
}

/** What happened since the owner last looked, at the top of Overview in the morning, until Dismiss or noon. */
export function Brief() {
  const { api, setScreen } = useStore();
  const flow = useTaskFlow();
  const [brief, setBrief] = useState<BriefData | null>(null);
  useEffect(() => {
    let live = true;
    const load = () => void api.brief().then((b) => live && setBrief(b), () => {});
    load();
    const off = api.onBriefChanged(load);
    // Read again now and then, so the card goes at noon and picks up what changed without an event.
    const t = window.setInterval(load, 5 * 60_000);
    return () => {
      live = false;
      off();
      window.clearInterval(t);
    };
  }, [api]);
  if (!brief?.shown || brief.items.length === 0) return <>{flow.overlays}</>;
  const dismiss = async () => {
    setBrief(null);
    await api.dismissBrief();
  };
  const open = (i: BriefItem) => {
    if (i.kind === "run" || i.kind === "issue") flow.open(i.task_id);
    else if (i.kind === "container" || i.kind === "server_restart" || i.kind === "disk") setScreen("server");
    else if (i.kind === "site") setScreen("sites");
    else setScreen("printer");
  };
  return (
    <>
      <Panel
        label="Morning brief"
        className="shrink-0"
        title="Since you last looked"
        action={
          <Button variant="ghost" onClick={() => void dismiss()}>
            Dismiss
          </Button>
        }
      >
        <ul className="border-t border-line">
          {brief.items.map((i, n) => (
            <li key={n} className="border-b border-line last:border-b-0">
              <button type="button" onClick={() => open(i)} className="flex h-8 w-full items-center gap-2 px-3 text-left text-[12px] hover:bg-raised">
                <span className="w-10 shrink-0 font-mono text-[11px] text-text-3">{i.kind === "container" || i.kind === "server_restart" || i.kind === "site" ? "" : clock(i.at)}</span>
                {i.kind === "run" && i.night && <Moon />}
                <span className={`truncate ${(i.kind === "run" || i.kind === "print") && !i.ok ? "text-text" : "text-text-2"}`}>{briefText(i)}</span>
              </button>
            </li>
          ))}
        </ul>
      </Panel>
      {flow.overlays}
    </>
  );
}
