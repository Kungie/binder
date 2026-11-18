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
  if (pane.kind === "attach") return pane.session;
  if (pane.kind === "ssh") return `ssh-${slug(pane.target)}`;
  return pane.project ? `binder-claude-${slug(pane.project)}` : "binder-claude";
}

export const projectPane = (p: Project): Pane => ({ kind: "claude", project: p.name, dir: p.agent_dir });

export const STATE_WORD: Record<PaneState | "off" | "error", string> = { working: "Working", waiting: "Waiting", idle: "Idle", exited: "Exited", off: "Off", error: "Error" };

/** A session's dot and word color: green working, yellow waiting on the owner, red ended or unreachable, gray idle. */
export const STATE_COLOR: Record<PaneState | "off" | "error", { dot: string; text: string }> = {
  working: { dot: "bg-ok", text: "text-ok" },
  waiting: { dot: "bg-st-progress", text: "text-st-progress" },
  exited: { dot: "bg-bad", text: "text-bad" },
  error: { dot: "bg-bad", text: "text-bad" },
  idle: { dot: "bg-text-3", text: "text-text-3" },
  off: { dot: "border border-line-strong", text: "text-text-3" },
};

/** Tokens as people read them: 950, 12k, 1.2M. */
export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${Math.round(n / 1000)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}
