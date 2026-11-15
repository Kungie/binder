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

export const DEFAULT_LAYOUT: OverviewLayout = { left: ["numbers", "done_today"], right: ["server", "cloud", "printer", "open_tasks"], split: 50, heights: {} };

/** How far the column split and a widget's height may go. */
export const SPLIT_MIN = 30;
export const SPLIT_MAX = 70;
export const HEIGHT_MIN = 80;
export const HEIGHT_MAX = 1200;
const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, Math.round(n)));
export const clampSplit = (n: number) => clamp(n, SPLIT_MIN, SPLIT_MAX);
export const clampHeight = (n: number) => clamp(n, HEIGHT_MIN, HEIGHT_MAX);

/** Known ids only, each once, left first; the split kept in range and heights only for widgets shown. */
export function clean(l: OverviewLayout): OverviewLayout {
  const seen = new Set<string>();
  const keep = (ids: string[]) => ids.filter((id) => id in WIDGETS && !seen.has(id) && (seen.add(id), true));
  const left = keep(l.left ?? []);
  const right = keep(l.right ?? []);
  const heights: Record<string, number> = {};
  for (const [id, h] of Object.entries(l.heights ?? {})) if (seen.has(id) && Number.isFinite(h)) heights[id] = clampHeight(h);
  return { left, right, split: clampSplit(l.split ?? 50), heights };
}

/** Moves a widget before another one, or to the end of a column when `before` is null. */
export function move(l: OverviewLayout, id: string, side: "left" | "right", before: string | null): OverviewLayout {
  const out = { ...l, left: l.left.filter((x) => x !== id), right: l.right.filter((x) => x !== id) };
  const col = out[side];
  const at = before === null ? -1 : col.indexOf(before);
  if (at === -1) col.push(id);
  else col.splice(at, 0, id);
  return out;
}

/** One keyboard step: up and down within the column, left and right to the other column at the same place. */
export function step(l: OverviewLayout, id: string, dir: "up" | "down" | "left" | "right"): OverviewLayout {
  const side = l.left.includes(id) ? "left" : "right";
  const col = l[side];
  const at = col.indexOf(id);
  if (dir === "up" || dir === "down") {
    const to = dir === "up" ? at - 1 : at + 1;
    if (to < 0 || to >= col.length) return l;
    const next = [...col];
    [next[at], next[to]] = [next[to], next[at]];
    return { ...l, [side]: next };
  }
  const other = dir === "left" ? "left" : "right";
  if (other === side) return l;
  const target = l[other];
  return move(l, id, other, target[Math.min(at, target.length)] ?? null);
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
