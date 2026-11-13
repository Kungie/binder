import { useEffect, useState } from "react";
import { Panel } from "../components/Panel";
import { ServerTargetForm } from "../components/ServerTargetForm";
import { Button, Field, Notice, fieldClass } from "../components/ui";
import { pickFolder } from "../lib/pick";
import { useStore } from "../lib/store";
import { useVault } from "../lib/useVault";

function ClaudeCode() {
  const { api } = useStore();
  const [on, setOn] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void api.claudeHooks().then(setOn);
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
          <span className="text-text-2">{on ? "Connected" : "Not connected"}</span>
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
      <div className="flex max-w-[640px] flex-col gap-4 px-5">
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
