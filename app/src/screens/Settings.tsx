import { useEffect, useState } from "react";
import { Panel } from "../components/Panel";
import { ServerTargetForm } from "../components/ServerTargetForm";
import { Button, Field, Notice, fieldClass } from "../components/ui";
import { pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { useVault } from "../lib/useVault";
import { ago } from "../lib/time";

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
  const [note, setNote] = useState<string | null>(null);
  useEffect(() => {
    void api.getConfig().then((c) => {
      setPath(c.cloud);
      setShare(c.cloud_share ?? "");
    });
  }, [api]);
  const save = async () => {
    const address = share.trim();
    if (address && !address.startsWith("smb://")) return setNote("The share address starts with smb://.");
    await api.setConfig({ ...(await api.getConfig()), cloud: (path ?? "").trim() || "/Volumes/cloud", cloud_share: address || null });
    setNote("Saved");
  };
  if (path === null) return null;
  return (
    <Panel title="Cloud">
      <div className="flex flex-col gap-3 border-t border-line p-3">
        <Field label="Mount folder">
          <input aria-label="Cloud mount folder" className={`${fieldClass} font-mono`} value={path} onChange={(e) => (setPath(e.target.value), setNote(null))} />
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
            <Notice>Binder uses your ssh keys and ~/.ssh/config. It never stores a password.</Notice>
          </div>
        </Panel>
      </div>
    </div>
  );
}
