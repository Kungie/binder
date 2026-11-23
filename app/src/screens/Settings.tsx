import { ZOOMS } from "../lib/zoom";
import { useEffect, useState } from "react";
import { BinderPanel } from "../components/BinderPanel";
import { Panel } from "../components/Panel";
import { ServerTargetForm } from "../components/ServerTargetForm";
import { Button, Field, Notice, Switch, fieldClass } from "../components/ui";
import { pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { useVault } from "../lib/useVault";
import { ago } from "../lib/time";
import { SETUP_EVENT } from "../components/Welcome";

/** The agent on the server: picking up its tasks by day, and the crontab line that runs night tasks. */
function AgentWork() {
  const { api } = useStore();
  const [pickUp, setPickUp] = useState<boolean | null>(null);
  const [night, setNight] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void api.getConfig().then((c) => setPickUp(c.pick_up));
    void api.nightState().then(setNight, () => setNight(null));
  }, [api]);
  const savePickUp = async (on: boolean) => {
    setPickUp(on);
    await api.setConfig({ ...(await api.getConfig()), pick_up: on });
  };
  const changeNight = async (run: () => Promise<boolean>) => {
    setBusy(true);
    setError(null);
    try {
      setNight(await run());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <div className="flex items-center gap-2 text-[12px]">
        <span className="text-text-2">Pick up agent tasks</span>
        <span className="text-text-3">every 10 minutes, one at a time</span>
        {pickUp !== null && <Switch label="Pick up agent tasks" checked={pickUp} onChange={(on) => void savePickUp(on)} />}
      </div>
      {night !== null && (
        <div className="flex items-center gap-2 text-[12px]">
          <span className="text-text-2">Night tasks</span>
          <span className="text-text-3">{night ? "Runs at 03:00 on the server" : "Not set up"}</span>
          <Button className="ml-auto" disabled={busy} onClick={() => void changeNight(night ? api.nightRemove : api.nightSetup)}>
            {night ? "Remove" : "Set up"}
          </Button>
        </div>
      )}
      {error && <Notice>{error}</Notice>}
    </>
  );
}

/** Interface size, for large displays: the whole window scales. */
function Display() {
  const { zoom, setZoom } = useStore();
  return (
    <Panel title="Display">
      <div className="flex items-center gap-2 border-t border-line p-3 text-[12px]">
        <span className="text-text-2">Interface size</span>
        <div className="ml-auto flex h-7 items-center rounded-control border border-line bg-card p-0.5">
          {ZOOMS.map((z) => (
            <button
              key={z}
              type="button"
              aria-pressed={z === zoom}
              onClick={() => void setZoom(z)}
              className={`h-6 rounded-[6px] px-2 font-mono text-[11px] ${z === zoom ? "bg-raised text-text" : "text-text-3 hover:text-text"}`}
            >
              {z}%
            </button>
          ))}
        </div>
      </div>
    </Panel>
  );
}

/** A Mac notification when a print finishes or fails. */
function PrintSwitch() {
  const { api } = useStore();
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    void api.getConfig().then((c) => setOn(c.print_notify));
  }, [api]);
  if (on === null) return null;
  const save = async (next: boolean) => {
    setOn(next);
    await api.setConfig({ ...(await api.getConfig()), print_notify: next });
  };
  return (
    <div className="flex items-center gap-2 text-[12px]">
      <span className="text-text-2">Notify me about prints</span>
      <span className="text-text-3">when one is done or fails</span>
      <Switch label="Notify me about prints" checked={on} onChange={(next) => void save(next)} />
    </div>
  );
}

/** Mac notifications when a container on the server stops, keeps restarting or turns unhealthy. */
function WatchSwitch() {
  const { api } = useStore();
  const [on, setOn] = useState<boolean | null>(null);
  useEffect(() => {
    void api.getConfig().then((c) => setOn(c.watch));
  }, [api]);
  if (on === null) return null;
  const save = async (next: boolean) => {
    setOn(next);
    await api.setConfig({ ...(await api.getConfig()), watch: next });
  };
  return (
    <div className="flex items-center gap-2 text-[12px]">
      <span className="text-text-2">Notify me about containers</span>
      <span className="text-text-3">when one stops, keeps restarting or turns unhealthy</span>
      <Switch label="Notify me about containers" checked={on} onChange={(next) => void save(next)} />
    </div>
  );
}

function ClaudeCode() {
  const { api } = useStore();
  const [on, setOn] = useState<boolean | null>(null);
  const [last, setLast] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void api.claudeHooks().then(setOn);
    void api.lastDone().then(setLast, () => {});
  }, [api]);
  const toggle = async () => {
    setError(null);
    try {
      setOn(await api.setClaudeHooks(!on));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <Panel title="Claude Code" label="Claude Code">
      <div className="flex flex-col gap-3 border-t border-line p-3">
        <div className="flex items-center gap-2 text-[12px]">
          <span className={`h-1.5 w-1.5 rounded-full ${on ? "bg-text" : "border border-text-3"}`} aria-hidden="true" />
          <span className="text-text-2">{on ? (last ? `On, last line ${ago(last)}` : "On, no lines yet") : "Off"}</span>
          {on !== null && (
            <Button className="ml-auto" onClick={() => void toggle()}>
              {on ? "Disconnect" : "Connect"}
            </Button>
          )}
        </div>
        {error && <Notice>{error}</Notice>}
        <AgentWork />
        <Notice>
          Adds three hooks to ~/.claude/settings.json. When a session in a project's repo has new talk, Haiku picks out up to three key decisions for the
          project's done log. It runs with your own Claude plan, after secrets are masked.
        </Notice>
      </div>
    </Panel>
  );
}

/** Where Cloud is mounted on this Mac and the share address Connect opens. */
function CloudSettings() {
  const { api } = useStore();
  const [path, setPath] = useState<string | null>(null);
  const [share, setShare] = useState("");
  const [remote, setRemote] = useState("");
  const [fast, setFast] = useState<"ssh" | "smb" | null>(null);
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    void api.getConfig().then((c) => {
      setPath(c.cloud);
      setShare(c.cloud_share ?? "");
      setRemote(c.cloud_remote);
    });
    // Only known while the share is mounted.
    void api.cloudFastPath().then(setFast, () => setFast(null));
  }, [api]);
  const save = async () => {
    const address = share.trim();
    if (address && !address.startsWith("smb://")) return setNote("The share address starts with smb://.");
    await api.setConfig({ ...(await api.getConfig()), cloud: (path ?? "").trim() || "/Volumes/cloud", cloud_share: address || null, cloud_remote: remote.trim() || "/srv/cloud" });
    setNote("Saved");
  };
  if (path === null) return null;
  return (
    <Panel title="Cloud">
      <div className="flex flex-col gap-3 border-t border-line p-3">
        <Field label="Mount folder">
          <input aria-label="Cloud mount folder" className={`${fieldClass} font-mono`} value={path} onChange={(e) => (setPath(e.target.value), setNote(null))} />
        </Field>
        <Field label="Folder on the server">
          <input aria-label="Cloud folder on the server" className={`${fieldClass} font-mono`} value={remote} onChange={(e) => (setRemote(e.target.value), setNote(null))} />
        </Field>
        <Field label="Share address">
          <div className="flex gap-2">
            <input
              aria-label="Cloud share address"
              placeholder="smb://server/cloud"
              className={`${fieldClass} font-mono`}
              value={share}
              onChange={(e) => (setShare(e.target.value), setNote(null))}
            />
            <Button aria-label="Save Cloud settings" onClick={() => void save()}>Save</Button>
          </div>
        </Field>
        {note && <span className="text-[12px] text-text-3">{note}</span>}
        {fast && <span className="text-[12px] text-text-2">{fast === "ssh" ? "Fast path: SSH" : "Fast path: off, using SMB"}</span>}
        <Notice>Cloud is a folder on the home server, shared over SMB like the vault. Connect in Finder once, save the password in the Keychain and add the share to Login Items. Your phone reaches it from Files, Connect to Server, over Tailscale.</Notice>
      </div>
    </Panel>
  );
}

export function Settings() {
  const { api } = useStore();
  const vault = useVault();
  const [path, setPath] = useState("");
  const [saved, setSaved] = useState(false);
  const [server, setServer] = useState<string | null | undefined>(undefined);
  const [serverSaved, setServerSaved] = useState(false);

  useEffect(() => setPath(vault.path), [vault.path]);
  useEffect(() => {
    void api.getConfig().then((c) => setServer(c.server));
  }, [api]);

  const save = async () => {
    await api.setConfig({ ...(await api.getConfig()), vault: path.trim() });
    setSaved(true);
    vault.recheck();
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Settings</h1>
      </header>
      <div className="flex max-w-[960px] flex-col gap-4 px-5">
        <BinderPanel />
        <Display />
        <Panel title="Obsidian vault">
          <div className="flex flex-col gap-3 border-t border-line p-3">
            <Field label="Vault folder">
              <div className="flex gap-2">
                <input
                  aria-label="Vault folder"
                  className={`${fieldClass} font-mono`}
                  value={path}
                  onChange={(e) => (setPath(e.target.value), setSaved(false))}
                />
                <Button variant="ghost" onClick={() => void pickFolder().then((f) => f && (setPath(f), setSaved(false)))}>
                  Choose
                </Button>
                <Button onClick={() => void save()}>Save</Button>
              </div>
            </Field>
            <div className="flex items-center gap-2 text-[12px]">
              <span className={`h-1.5 w-1.5 rounded-full ${vault.connected ? "bg-text" : "border border-text-3"}`} aria-hidden="true" />
              <span className="text-text-2">{vault.connected ? "Connected" : "Not connected"}</span>
              {saved && <span className="text-text-3">Saved</span>}
            </div>
            <Notice>The vault lives on the home server. Connect to its share in Finder once, save the password in the Keychain and add it to Login Items so it mounts at login.</Notice>
          </div>
        </Panel>
        <CloudSettings />
        <ClaudeCode />
        <Panel title="Home server">
          <div className="flex flex-col gap-3 border-t border-line p-3">
            {server !== undefined && <ServerTargetForm initial={server ?? ""} onSaved={() => setServerSaved(true)} />}
            {serverSaved && <span className="text-[12px] text-text-3">Saved</span>}
            <WatchSwitch />
            <Notice>Binder uses your ssh keys and ~/.ssh/config. It never stores a password.</Notice>
          </div>
        </Panel>
        <Panel title="3D Printer">
          <div className="flex flex-col gap-3 border-t border-line p-3">
            <PrintSwitch />
          </div>
        </Panel>
        <SetupPanel />
      </div>
    </div>
  );
}

function SetupPanel() {
  const { api } = useStore();
  const [note, setNote] = useState<string | null>(null);
  return (
    <Panel title="Setup">
      <div className="flex flex-wrap items-center gap-2 border-t border-line p-3">
        <Button onClick={() => window.dispatchEvent(new Event(SETUP_EVENT))}>Run setup again</Button>
        <Button
          onClick={() =>
            void api.getConfig().then(async (c) => {
              await api.setConfig({ ...c, tips_hidden: [] });
              setNote("Each screen shows its guide again the next time it opens.");
            })
          }
        >
          Show guides again
        </Button>
        {note && <span className="text-[12px] text-text-3">{note}</span>}
      </div>
    </Panel>
  );
}
