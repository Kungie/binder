import type { ReactNode } from "react";
import { type Screen, useStore } from "../lib/store";
import { isClosed } from "../lib/types";
import { ProjectDot } from "./ProjectDot";

function Item({ active, onClick, icon, children, count, dot }: { active: boolean; onClick(): void; icon: ReactNode; children: ReactNode; count?: number; dot?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={`flex h-7 w-full items-center gap-2.5 rounded-control px-2 text-left transition-colors duration-[120ms] ${
        active ? "bg-raised text-text" : "text-text-2 hover:bg-raised/60 hover:text-text"
      }`}
    >
      <span className="flex w-4 justify-center text-text-3">{icon}</span>
      <span className="truncate">{children}</span>
      {dot && <span role="img" aria-label="Needs attention" className="ml-auto size-1.5 shrink-0 rounded-full bg-urgent" />}
      {count !== undefined && <span className={`${dot ? "" : "ml-auto "}font-mono text-[11px] text-text-3`}>{count}</span>}
    </button>
  );
}

const Glyph = ({ d }: { d: string }) => (
  <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" aria-hidden="true">
    <path d={d} strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const NAV: { screen: Screen; label: string; d: string }[] = [
  { screen: "overview", label: "Overview", d: "M2 2h4v4H2zM8 2h4v4H8zM2 8h4v4H2zM8 8h4v4H8z" },
  { screen: "tasks", label: "Tasks", d: "M7 1.8a5.2 5.2 0 1 1 0 10.4A5.2 5.2 0 0 1 7 1.8zM4.8 7.1l1.5 1.5 3-3.2" },
  { screen: "projects", label: "Projects", d: "M1.8 3.5h4l1.2 1.4h5.2v6.6H1.8z" },
  { screen: "devices", label: "Devices", d: "M2 3h10v6.5H2zM5 12h4M7 9.5V12" },
  { screen: "server", label: "Server", d: "M2 2.5h10v3.5H2zM2 8h10v3.5H2zM4.2 4.25h.01M4.2 9.75h.01" },
  { screen: "cloud", label: "Cloud", d: "M4 11h6.2a2.8 2.8 0 0 0 .4-5.57A3.8 3.8 0 0 0 3.3 6.2 2.4 2.4 0 0 0 4 11z" },
  { screen: "printer", label: "Printer", d: "M3.5 5V1.8h7V5M2 5h10v5H2zM4 8.5h6v3.7H4z" },
];

export function Sidebar() {
  const { screen, setScreen, tasks, projects, openProject, selectedProject } = useStore();
  const open = tasks.filter((t) => !isClosed(t.status)).length;
  const attention = tasks.some((t) => t.attention);
  return (
    <nav aria-label="Main" className="flex h-full w-[208px] shrink-0 flex-col border-r border-line px-2 pb-3 pt-10" data-tauri-drag-region>
      <div className="mb-3 flex items-center gap-2 px-2" data-tauri-drag-region>
        <span className="h-2.5 w-2.5 rounded-[3px] bg-text" aria-hidden="true" />
        <span className="font-medium">Binder</span>
        <kbd className="ml-auto rounded-[4px] border border-line px-1.5 font-mono text-[11px] text-text-3">⌘K</kbd>
      </div>
      <div className="flex flex-col gap-px">
        {NAV.map((n) => (
          <Item key={n.screen} active={screen === n.screen} onClick={() => setScreen(n.screen)} icon={<Glyph d={n.d} />} count={n.screen === "tasks" ? open : undefined} dot={n.screen === "tasks" && attention}>
            {n.label}
          </Item>
        ))}
      </div>
      <div className="mb-1 mt-5 px-2 text-[11px] text-text-3">Projects</div>
      <div className="flex min-h-0 flex-col gap-px overflow-y-auto">
        {projects
          .filter((p) => p.state === "active" && !p.archived)
          .map((p) => (
            <Item
              key={p.id}
              active={screen === "projects" && selectedProject === p.id}
              onClick={() => openProject(p.id)}
              icon={<ProjectDot color={p.color} />}
            >
              {p.name}
            </Item>
          ))}
      </div>
      <div className="mt-auto">
        <Item active={screen === "settings"} onClick={() => setScreen("settings")} icon={<Glyph d="M7 4.6a2.4 2.4 0 1 1 0 4.8 2.4 2.4 0 0 1 0-4.8zM7 1v1.6M7 11.4V13M1 7h1.6M11.4 7H13M2.8 2.8l1.1 1.1M10.1 10.1l1.1 1.1M2.8 11.2l1.1-1.1M10.1 3.9l1.1-1.1" />}>
          Settings
        </Item>
      </div>
    </nav>
  );
}
