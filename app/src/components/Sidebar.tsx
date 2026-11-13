import type { ReactNode } from "react";
import { type Screen, useStore } from "../lib/store";
import { isClosed } from "../lib/types";

function Item({ active, onClick, icon, children, count }: { active: boolean; onClick(): void; icon: ReactNode; children: ReactNode; count?: number }) {
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
      {count !== undefined && <span className="ml-auto font-mono text-[11px] text-text-3">{count}</span>}
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
];

export function Sidebar() {
  const { screen, setScreen, tasks, projects, openProject, selectedProject } = useStore();
  const open = tasks.filter((t) => !isClosed(t.status)).length;
  return (
    <nav aria-label="Main" className="flex h-full w-[220px] shrink-0 flex-col border-r border-line px-2 pb-3 pt-10" data-tauri-drag-region>
      <div className="mb-3 flex items-center gap-2 px-2" data-tauri-drag-region>
        <span className="h-2.5 w-2.5 rounded-[3px] bg-text" aria-hidden="true" />
        <span className="font-medium">Binder</span>
        <kbd className="ml-auto rounded-[4px] border border-line px-1.5 font-mono text-[11px] text-text-3">⌘K</kbd>
      </div>
      <div className="flex flex-col gap-px">
        {NAV.map((n) => (
          <Item key={n.screen} active={screen === n.screen} onClick={() => setScreen(n.screen)} icon={<Glyph d={n.d} />} count={n.screen === "tasks" ? open : undefined}>
            {n.label}
          </Item>
        ))}
      </div>
      <div className="mb-1 mt-5 px-2 text-[11px] text-text-3">Projects</div>
      <div className="flex min-h-0 flex-col gap-px overflow-y-auto">
        {projects
          .filter((p) => p.state === "active")
          .map((p) => (
            <Item
              key={p.id}
              active={screen === "projects" && selectedProject === p.id}
              onClick={() => openProject(p.id)}
              icon={<span className="h-1.5 w-1.5 rounded-full bg-text" />}
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
