import type { Api } from "./lib/api";
import { type Screen, StoreProvider, useStore } from "./lib/store";
import { Sidebar } from "./components/Sidebar";
import { ZOOMS } from "./lib/zoom";
import { NewTask } from "./components/NewTask";
import { Tasks } from "./screens/Tasks";
import { Projects } from "./screens/Projects";
import { Settings } from "./screens/Settings";
import { NewProject } from "./components/NewProject";
import { CommandPalette } from "./components/CommandPalette";
import { Overview } from "./screens/Overview";
import { Devices } from "./screens/Devices";
import { Server } from "./screens/Server";
import { Printer } from "./screens/Printer";
import { Cloud } from "./screens/Cloud";
import { Transfers } from "./components/Transfers";
import { Agents } from "./screens/Agents";
import { useEffect, useState } from "react";

function Screens() {
  const { screen } = useStore();
  switch (screen) {
    case "overview":
      return <Overview />;
    case "tasks":
      return <Tasks />;
    case "projects":
      return <Projects />;
    case "devices":
      return <Devices />;
    case "server":
      return <Server />;
    case "cloud":
      return <Cloud />;
    case "printer":
      return <Printer />;
    case "settings":
      return <Settings />;
    case "agents":
      return null;
  }
}

function useZoomKeys() {
  const { zoom, setZoom } = useStore();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey) return;
      const i = ZOOMS.indexOf(zoom);
      const next = e.key === "=" || e.key === "+" ? ZOOMS[Math.min(i + 1, ZOOMS.length - 1)] : e.key === "-" ? ZOOMS[Math.max(i - 1, 0)] : e.key === "0" ? 100 : null;
      if (next === null) return;
      e.preventDefault();
      if (next !== zoom) void setZoom(next).catch(() => {});
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [zoom, setZoom]);
}

export function Shell() {
  const { creatingTask, creatingProject, screen } = useStore();
  useZoomKeys();
  // Agents stays mounted once opened, so its terminals keep their connection and screen.
  const [agentsSeen, setAgentsSeen] = useState(false);
  if (screen === "agents" && !agentsSeen) setAgentsSeen(true);
  return (
    <div className="flex h-full">
      <Sidebar />
      {/* overflow-hidden: a screen rising in from a few pixels down must not give the window a scrollbar. */}
      <main className="relative flex min-w-0 flex-1 flex-col overflow-hidden">
        <div className="h-3 shrink-0" data-tauri-drag-region />
        {/* Keyed by screen, so each one arrives with its own animation. */}
        {screen !== "agents" && (
          <div key={screen} data-testid="screen" data-screen={screen} className="screen-in flex min-h-0 flex-1 flex-col">
            <Screens />
          </div>
        )}
        {agentsSeen && <Agents shown={screen === "agents"} />}
        <Transfers />
      </main>
      {creatingTask && <NewTask />}
      {creatingProject && <NewProject />}
      <CommandPalette />
    </div>
  );
}

export default function App({ api, initialScreen }: { api: Api; initialScreen?: Screen }) {
  return (
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>
  );
}
