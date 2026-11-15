import { useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { OverviewLayout, Site } from "../lib/types";
import { Panel } from "./Panel";

/** Every widget Overview can show, by id, with the name the owner sees. */
export const WIDGETS: Record<string, string> = {
  numbers: "Numbers",
  done_today: "Done today",
  open_tasks: "Open tasks",
  server: "Server",
  cloud: "Cloud",
  printer: "Printer",
  sites: "Sites",
  github: "GitHub",
};

/** Widgets that fill what is left of their column's height. */
export const FILLS = new Set(["done_today", "open_tasks"]);

export const DEFAULT_LAYOUT: OverviewLayout = { left: ["numbers", "done_today"], right: ["server", "cloud", "printer", "open_tasks"] };

/** Known ids only, each once, left first. */
export function clean(l: OverviewLayout): OverviewLayout {
  const seen = new Set<string>();
  const keep = (ids: string[]) => ids.filter((id) => id in WIDGETS && !seen.has(id) && (seen.add(id), true));
  return { left: keep(l.left ?? []), right: keep(l.right ?? []) };
}

/** Moves a widget before another one, or to the end of a column when `before` is null. */
export function move(l: OverviewLayout, id: string, side: "left" | "right", before: string | null): OverviewLayout {
  const out = { left: l.left.filter((x) => x !== id), right: l.right.filter((x) => x !== id) };
  const col = out[side];
  const at = before === null ? -1 : col.indexOf(before);
  if (at === -1) col.push(id);
  else col.splice(at, 0, id);
  return out;
}

const OpenButton = ({ onClick }: { onClick(): void }) => (
  <button type="button" className="text-[12px] text-text-3 hover:text-text" onClick={onClick}>
    Open
  </button>
);

/** How many sites run, by their containers, with a dot for each. */
export function SitesWidget() {
  const { api, setScreen } = useStore();
  const [sites, setSites] = useState<Site[] | null>(null);
  useEffect(() => {
    let live = true;
    void api.sites().then((s) => live && setSites(s), () => live && setSites([]));
    return () => {
      live = false;
    };
  }, [api]);
  const up = (s: Site) => s.containers.length > 0 && s.containers.every((c) => c.state === "running");
  return (
    <Panel label="Sites" title="Sites" action={<OpenButton onClick={() => setScreen("sites")} />}>
      <div className="flex flex-col gap-2 border-t border-line px-3 py-2.5 text-[12px]">
        {sites === null ? (
          <span className="text-text-3">Reading…</span>
        ) : (
          <>
            <span className="text-text">{`${sites.filter(up).length} of ${sites.length} up`}</span>
            <span className="flex flex-wrap gap-x-3 gap-y-1">
              {sites.map((s) => (
                <span key={s.name} className="flex items-center gap-1.5 text-text-2">
                  <span className={`h-1.5 w-1.5 rounded-full ${up(s) ? "bg-ok" : "bg-bad"}`} aria-hidden="true" />
                  {s.name}
                </span>
              ))}
            </span>
          </>
        )}
      </div>
    </Panel>
  );
}

const count = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

/** What waits for the owner on GitHub. */
export function GithubWidget() {
  const { api, setScreen } = useStore();
  const [text, setText] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void api.ghList("needs_you", null).then(
      (l) => live && setText(l.pulls.length + l.issues.length === 0 ? "Nothing needs you." : `${count(l.pulls.length, "pull request")}, ${count(l.issues.length, "issue")} need you`),
      () => live && setText("GitHub isn't reachable."),
    );
    return () => {
      live = false;
    };
  }, [api]);
  return (
    <Panel label="GitHub" title="GitHub" action={<OpenButton onClick={() => setScreen("github")} />}>
      <p className="border-t border-line px-3 py-2.5 text-[12px] text-text">{text ?? "Reading…"}</p>
    </Panel>
  );
}
