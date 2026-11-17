import type { Pane, PaneState, Project } from "./types";

/** A project's name as part of a tmux session name, as the core makes it. */
export function slug(name: string): string {
  let out = "";
  for (const c of name.toLowerCase()) {
    if (/[a-z0-9]/.test(c)) out += c;
    else if (!out.endsWith("-")) out += "-";
  }
  out = out.replace(/^-+|-+$/g, "");
  return out || "project";
}

export function paneSession(pane: Pane): string {
  if (pane.kind === "ollama") return "binder-ollama";
  return pane.project ? `binder-claude-${slug(pane.project)}` : "binder-claude";
}

export const projectPane = (p: Project): Pane => ({ kind: "claude", project: p.name, dir: p.agent_dir });

export const STATE_WORD: Record<PaneState | "off", string> = { working: "Working", waiting: "Waiting", idle: "Idle", exited: "Exited", off: "Off" };

/** Tokens as people read them: 950, 12k, 1.2M. */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${Math.round(n / 1000)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}
