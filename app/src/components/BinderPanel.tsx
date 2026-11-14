import { useState } from "react";
import { plainText } from "../lib/plain";
import { useStore } from "../lib/store";
import type { InstallProgress } from "../lib/types";
import { Panel } from "./Panel";
import { Button } from "./ui";

type Install = { kind: "idle" } | { kind: "working"; progress: InstallProgress | null } | { kind: "failed" };

function installLabel(install: Install): string {
  if (install.kind !== "working" || !install.progress) return install.kind === "working" ? "Downloading…" : "Install and restart";
  const p = install.progress;
  if (p.kind === "installing") return "Installing…";
  return p.percent === null ? "Downloading…" : `Downloading ${p.percent}%`;
}

/** The running version and updates from GitHub releases. First in Settings. */
export function BinderPanel() {
  const { api, app, update, checkUpdate, transferring } = useStore();
  const [install, setInstall] = useState<Install>({ kind: "idle" });
  if (!app) return null;

  const start = async () => {
    setInstall({ kind: "working", progress: null });
    try {
      await api.installUpdate((progress) => setInstall({ kind: "working", progress }));
    } catch {
      setInstall({ kind: "failed" });
    }
  };
  const working = install.kind === "working";

  return (
    <Panel title="Binder">
      <div className="flex flex-col gap-3 border-t border-line p-3 text-[12px]">
        <div className="flex items-center gap-2">
          <span className="font-mono text-text-2">Version {app.version}</span>
          {app.updates && (
            <Button variant="ghost" className="ml-auto" disabled={update.kind === "checking" || working} onClick={() => void checkUpdate(false)}>
              {update.kind === "checking" ? "Checking…" : "Check for updates"}
            </Button>
          )}
        </div>
        {!app.updates && <p className="text-text-2">Updates are off in development builds.</p>}
        {update.kind === "current" && <p className="text-text-2">Binder is up to date.</p>}
        {update.kind === "failed" && <p className="text-bad">Couldn't check for updates.</p>}
        {update.kind === "available" && (
          <div className="flex flex-col gap-2">
            <p>Binder {update.info.version} is available.</p>
            {update.info.notes.trim() && <p className="line-clamp-6 whitespace-pre-line text-text-2">{plainText(update.info.notes)}</p>}
            <div className="flex items-center gap-2">
              <Button variant="primary" disabled={working || transferring} onClick={() => void start()}>
                {installLabel(install)}
              </Button>
              {transferring && !working && <span className="text-text-2">Wait for the Cloud transfer to finish.</span>}
            </div>
            {install.kind === "failed" && <p className="text-bad">The update couldn't be installed.</p>}
          </div>
        )}
      </div>
    </Panel>
  );
}
