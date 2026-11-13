import { Command } from "cmdk";
import { useEffect, useState } from "react";
import { type Screen, useStore } from "../lib/store";

/** ⌘K anywhere: go to a screen, create something, or open a project. */
export function CommandPalette() {
  const { setScreen, setCreatingTask, setCreatingProject, projects, openProject } = useStore();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!open) return null;

  const run = (action: () => void) => () => {
    setOpen(false);
    action();
  };
  const go = (s: Screen) => run(() => setScreen(s));
  const item = "flex h-8 cursor-default items-center rounded-[4px] px-2 text-[13px] text-text-2 data-[selected=true]:bg-line-strong data-[selected=true]:text-text";

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-[16vh]" onMouseDown={() => setOpen(false)}>
      <div role="dialog" aria-label="Command palette" className="fade-in w-[560px] overflow-hidden rounded-panel border border-line-strong bg-panel shadow-menu" onMouseDown={(e) => e.stopPropagation()}>
        <Command
          label="Command palette"
          loop
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Escape") setOpen(false);
          }}
        >
          <Command.Input autoFocus placeholder="Type a command or a project" className="h-11 w-full border-b border-line bg-transparent px-4 text-[13px] placeholder:text-text-3 focus:outline-none" />
          <Command.List className="max-h-[320px] overflow-y-auto p-1.5">
            <Command.Empty className="px-2 py-3 text-[12px] text-text-3">No match.</Command.Empty>
            <Command.Group heading="Go to" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:text-text-3">
              <Command.Item className={item} onSelect={go("overview")}>Go to Overview</Command.Item>
              <Command.Item className={item} onSelect={go("tasks")}>Go to Tasks</Command.Item>
              <Command.Item className={item} onSelect={go("projects")}>Go to Projects</Command.Item>
              <Command.Item className={item} onSelect={go("devices")}>Go to Devices</Command.Item>
              <Command.Item className={item} onSelect={go("server")}>Go to Server</Command.Item>
              <Command.Item className={item} onSelect={go("settings")}>Go to Settings</Command.Item>
            </Command.Group>
            <Command.Group heading="Create" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:text-text-3">
              <Command.Item className={item} onSelect={run(() => setCreatingTask(true))}>New task</Command.Item>
              <Command.Item className={item} onSelect={run(() => setCreatingProject(true))}>New project</Command.Item>
            </Command.Group>
            {projects.some((p) => p.state === "active") && (
              <Command.Group heading="Projects" className="[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:text-text-3">
                {projects
                  .filter((p) => p.state === "active")
                  .map((p) => (
                    <Command.Item key={p.id} value={`Open ${p.name}`} className={item} onSelect={run(() => openProject(p.id))}>
                      Open {p.name}
                    </Command.Item>
                  ))}
              </Command.Group>
            )}
          </Command.List>
        </Command>
      </div>
    </div>
  );
}
