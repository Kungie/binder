import type { Api } from "./lib/api";
import { type Screen, StoreProvider, useStore } from "./lib/store";
import { Sidebar } from "./components/Sidebar";
import { NewTask } from "./components/NewTask";
import { Tasks } from "./screens/Tasks";

function Placeholder({ title }: { title: string }) {
  return <div className="p-6 text-text-2">{title}</div>;
}

function Screens() {
  const { screen } = useStore();
  switch (screen) {
    case "overview":
      return <Placeholder title="Overview" />;
    case "tasks":
      return <Tasks />;
    case "projects":
      return <Placeholder title="Projects" />;
    case "settings":
      return <Placeholder title="Settings" />;
  }
}

export function Shell() {
  const { creatingTask } = useStore();
  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="relative flex min-w-0 flex-1 flex-col">
        <div className="h-3 shrink-0" data-tauri-drag-region />
        <Screens />
      </main>
      {creatingTask && <NewTask />}
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
