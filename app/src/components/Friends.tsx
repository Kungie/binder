import { type ReactNode, useCallback, useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { Friend, FriendsService } from "../lib/types";
import { Panel } from "./Panel";
import { Button, fieldClass } from "./ui";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const day = (t: number) => new Date(t * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });

type Open = { kind: "add" } | { kind: "oauth" } | { kind: "password"; name: string } | { kind: "remove"; name: string } | null;

const DOWNLOAD = "https://github.com/Kungie/binder/releases/latest/download/Binder-VPN.dmg";

/** What the owner adds to the tailnet policy: friends' devices reach the internet through exit nodes, nothing else. */
export const POLICY = `"tagOwners": {
  "tag:friend": ["autogroup:admin"],
},
"grants": [
  {"src": ["tag:friend"], "dst": ["autogroup:internet"], "ip": ["*"]},
],`;

/** A password nobody has to think up: 16 letters and digits from the system's random source, about 95 bits. */
export function newPassword(): string {
  const chars = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const out: string[] = [];
  const bytes = new Uint8Array(64);
  while (out.length < 16) {
    crypto.getRandomValues(bytes);
    for (const b of bytes) if (b < 232 && out.length < 16) out.push(chars[b % chars.length]);
  }
  return out.join("");
}

export const invite = (address: string | null, name: string, password: string) =>
  [`Binder VPN`, `1. Install Tailscale from tailscale.com/download and open it once.`, `2. Install Binder VPN: ${DOWNLOAD}`, `3. Sign in with:`, `   Server: ${address ?? "(ask me)"}`, `   User name: ${name}`, `   Password: ${password}`].join("\n");

const copy = (text: string) => void navigator.clipboard?.writeText(text).catch(() => {});

/** Friends who sign in to Binder VPN with a user name and a password, kept by the service on the server. */
export function Friends() {
  const { api } = useStore();
  const [service, setService] = useState<FriendsService | null>(null);
  const [list, setList] = useState<Friend[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<Open>(null);
  const [note, setNote] = useState<string | null>(null);
  const [address, setAddress] = useState<string | null>(null);
  const [dir, setDir] = useState("binder-vpn");

  const read = useCallback(async () => {
    try {
      const c = await api.getConfig();
      setAddress(c.vpn_address ?? null);
      setDir(c.vpn_dir ?? "binder-vpn");
      const s = await api.friendsStatus();
      setService(s);
      setList(s.installed ? await api.friendsList() : []);
      setError(null);
    } catch (e) {
      setError(message(e));
    }
  }, [api]);
  useEffect(() => void read(), [read]);

  const done = async (text: string | null) => {
    setOpen(null);
    setNote(text);
    await read();
  };

  const ready = !!service?.configured;
  return (
    <Panel
      title="Friends"
      label="Friends"
      action={
        ready && (
          <div className="flex items-center gap-1">
            <UpdateButton onDone={(t) => void done(t)} />
            <Button variant="ghost" onClick={() => setOpen(open?.kind === "oauth" ? null : { kind: "oauth" })}>
              Change client
            </Button>
            <Button variant="ghost" onClick={() => setOpen(open?.kind === "add" ? null : { kind: "add" })}>
              Add friend
            </Button>
          </div>
        )
      }
    >
      <div className="border-t border-line">
        <p className="px-3 py-2 text-[12px] leading-5 text-text-3">
          Friends sign in to Binder VPN with a user name and a password you make here and reach the internet through your exit nodes, nothing else on your tailnet.
        </p>
        {error && <p className="px-3 pb-2 text-[12px] text-bad">{error}</p>}
        {service && !service.installed && <InstallStep dir={dir} onInstalled={() => void done("The service is running.")} />}
        {service?.installed && !service.configured && (
          <>
            <PolicyStep />
            <OauthForm onSaved={() => void done("Client saved.")} />
          </>
        )}
        {ready && <AddressStep address={address} onSaved={() => void read()} />}
        {ready && open?.kind === "oauth" && <OauthForm onSaved={() => void done("Client saved.")} onCancel={() => setOpen(null)} />}
        {note && <p className="px-3 pb-2 text-[12px] text-ok">{note}</p>}
        {open?.kind === "add" && <FriendForm address={address} onDone={(ok) => void (ok ? read() : done(null))} />}
        {list.map((f) => (
          <div key={f.name} className="border-t border-line">
            <div className="flex items-center gap-3 px-3 py-2 text-[12px]">
              <span className="font-mono text-text">{f.name}</span>
              <span className="text-text-3">Added {day(f.created)}</span>
              <span className="flex-1 truncate text-text-3">{f.last_login ? `Signed in ${day(f.last_login)}` : "Never signed in"}</span>
              <Button variant="ghost" aria-label={`Reset password of ${f.name}`} onClick={() => setOpen({ kind: "password", name: f.name })}>
                Reset password
              </Button>
              <Button variant="ghost" aria-label={`Remove ${f.name}`} onClick={() => setOpen({ kind: "remove", name: f.name })}>
                Remove
              </Button>
            </div>
            {open?.kind === "password" && open.name === f.name && <PasswordForm name={f.name} address={address} onDone={(ok) => void (ok ? read() : done(null))} />}
            {open?.kind === "remove" && open.name === f.name && <RemoveForm name={f.name} onDone={() => void done(null)} />}
          </div>
        ))}
      </div>
    </Panel>
  );
}

function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <div className="flex gap-3 border-t border-line px-3 py-3">
      <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-line-strong font-mono text-[11px] text-text-2">{n}</span>
      <div className="flex min-w-0 flex-1 flex-col gap-2 text-[12px] leading-5 text-text-2">
        <span className="font-medium text-text">{title}</span>
        {children}
      </div>
    </div>
  );
}

function InstallStep({ dir, onInstalled }: { dir: string; onInstalled(): void }) {
  const { api } = useStore();
  const [folder, setFolder] = useState(dir);
  const { busy, error, submit } = useSubmit();
  useEffect(() => setFolder(dir), [dir]);
  return (
    <Step n={1} title="Run the sign-in service on your server">
      <span>
        Binder writes a small service (one Python file in Docker) to a folder under the server's home and starts it. It listens on <span className="font-mono text-text">127.0.0.1:8787</span> only. A
        folder that already has a compose file keeps it; it needs a <span className="font-mono text-text">vpn</span> service.
      </span>
      <div className="flex flex-wrap items-center gap-2">
        <input aria-label="Service folder" className={`${fieldClass} !w-48 font-mono`} value={folder} onChange={(e) => setFolder(e.target.value)} />
        <Button
          variant="primary"
          disabled={busy || !folder.trim()}
          onClick={() =>
            void submit(async () => {
              const c = await api.getConfig();
              await api.setConfig({ ...c, vpn_dir: folder.trim() });
              await api.friendsInstall();
            }).then((ok) => ok && onInstalled())
          }
        >
          {busy ? "Installing" : "Install service"}
        </Button>
      </div>
      {error && <span className="text-bad">{error}</span>}
    </Step>
  );
}

function PolicyStep() {
  return (
    <Step n={2} title="Keep friends to the internet">
      <span>In Tailscale's admin console, Access controls, add these to your policy. Friends' devices then reach the internet through your exit nodes and nothing else.</span>
      <pre className="overflow-x-auto rounded-control border border-line bg-card p-2 font-mono text-[11px] text-text">{POLICY}</pre>
      <div>
        <Button onClick={() => copy(POLICY)}>Copy policy</Button>
      </div>
    </Step>
  );
}

function AddressStep({ address, onSaved }: { address: string | null; onSaved(): void }) {
  const { api } = useStore();
  const [value, setValue] = useState(address ?? "");
  const [state, setState] = useState<"checking" | "up" | "down" | null>(null);
  const { busy, error, submit } = useSubmit();
  useEffect(() => setValue(address ?? ""), [address]);
  useEffect(() => {
    if (!address) return setState(null);
    let live = true;
    setState("checking");
    api.friendsReachable(address).then(
      (ok) => live && setState(ok ? "up" : "down"),
      () => live && setState("down"),
    );
    return () => {
      live = false;
    };
  }, [api, address]);
  return (
    <div className="flex flex-col gap-2 border-t border-line px-3 py-2 text-[12px] text-text-2">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-text-3">Friends sign in at</span>
        <input aria-label="Public address" placeholder="https://vpn.example.com" className={`${fieldClass} !w-72 font-mono`} value={value} onChange={(e) => setValue(e.target.value)} />
        <Button
          disabled={busy || value.trim() === (address ?? "")}
          onClick={() =>
            void submit(async () => {
              const c = await api.getConfig();
              await api.setConfig({ ...c, vpn_address: value.trim() || null });
            }).then((ok) => ok && onSaved())
          }
        >
          Save address
        </Button>
        {state && (
          <span className={`inline-flex items-center gap-1.5 ${state === "up" ? "text-ok" : state === "down" ? "text-bad" : "text-text-3"}`}>
            <span aria-hidden className={`size-1.5 rounded-full ${state === "up" ? "bg-ok" : state === "down" ? "bg-bad" : "bg-text-3"}`} />
            {state === "up" ? "Reachable" : state === "down" ? "Not reachable" : "Checking"}
          </span>
        )}
      </div>
      {!address && (
        <span className="leading-5">
          Make the service public over https. With Tailscale Funnel, run <span className="font-mono text-text">tailscale funnel --bg 8787</span> on the server and paste the address it prints. A
          Cloudflare Tunnel or a reverse proxy to <span className="font-mono text-text">127.0.0.1:8787</span> works too.
        </span>
      )}
      {error && <span className="text-bad">{error}</span>}
    </div>
  );
}

function UpdateButton({ onDone }: { onDone(text: string): void }) {
  const { api } = useStore();
  const { busy, error, submit } = useSubmit();
  return (
    <Button variant="ghost" disabled={busy} title={error ?? undefined} onClick={() => void submit(() => api.friendsInstall()).then((ok) => onDone(ok ? "The service is up to date." : "Couldn't update the service."))}>
      {busy ? "Updating" : "Update service"}
    </Button>
  );
}

function Invite({ address, name, password }: { address: string | null; name: string; password: string }) {
  const [copied, setCopied] = useState(false);
  const text = invite(address, name, password);
  return (
    <div className="flex flex-col gap-2 border-t border-line px-3 py-2">
      <span className="text-[12px] text-ok">Send this to {name}. The password isn't shown again.</span>
      <pre aria-label="Invite" className="whitespace-pre-wrap rounded-control border border-line bg-card p-2 font-mono text-[11px] text-text">{text}</pre>
      <div>
        <Button
          onClick={() => {
            copy(text);
            setCopied(true);
          }}
        >
          {copied ? "Copied" : "Copy invite"}
        </Button>
      </div>
    </div>
  );
}

function useSubmit() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await work();
      return true;
    } catch (e) {
      setError(message(e));
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, error, submit };
}

function FriendForm({ address, onDone }: { address: string | null; onDone(ok: boolean): void }) {
  const { api } = useStore();
  const [name, setName] = useState("");
  const [password, setPassword] = useState(newPassword);
  const [made, setMade] = useState<{ name: string; password: string } | null>(null);
  const { busy, error, submit } = useSubmit();
  if (made) return <Invite address={address} name={made.name} password={made.password} />;
  return (
    <form
      className="flex flex-wrap items-center gap-2 border-t border-line px-3 py-2"
      onSubmit={(e) => {
        e.preventDefault();
        const n = name.trim();
        void submit(() => api.friendAdd(n, password)).then((ok) => {
          if (ok) {
            setMade({ name: n, password });
            onDone(true);
          }
        });
      }}
    >
      <input aria-label="User name" placeholder="User name" autoFocus className={`${fieldClass} !w-40`} value={name} onChange={(e) => setName(e.target.value.toLowerCase())} />
      <input aria-label="Password" placeholder="Password, 8 or more" autoComplete="off" spellCheck={false} className={`${fieldClass} !w-48 font-mono`} value={password} onChange={(e) => setPassword(e.target.value)} />
      <Button variant="ghost" onClick={() => setPassword(newPassword())}>
        New password
      </Button>
      <Button type="submit" variant="primary" disabled={busy || !name.trim() || !password}>
        Add
      </Button>
      <Button variant="ghost" onClick={() => onDone(false)}>
        Cancel
      </Button>
      {error && <p className="w-full text-[12px] text-bad">{error}</p>}
    </form>
  );
}

function PasswordForm({ name, address, onDone }: { name: string; address: string | null; onDone(ok: boolean): void }) {
  const { api } = useStore();
  const [password, setPassword] = useState(newPassword);
  const [saved, setSaved] = useState(false);
  const { busy, error, submit } = useSubmit();
  if (saved) return <Invite address={address} name={name} password={password} />;
  return (
    <form
      className="flex flex-wrap items-center gap-2 px-3 pb-2"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(() => api.friendPassword(name, password)).then((ok) => {
          if (ok) {
            setSaved(true);
            onDone(true);
          }
        });
      }}
    >
      <input aria-label="New password" placeholder="New password" autoFocus autoComplete="off" spellCheck={false} className={`${fieldClass} !w-48 font-mono`} value={password} onChange={(e) => setPassword(e.target.value)} />
      <Button type="submit" variant="primary" disabled={busy || !password}>
        Save
      </Button>
      <Button variant="ghost" onClick={() => onDone(false)}>
        Cancel
      </Button>
      {error && <p className="w-full text-[12px] text-bad">{error}</p>}
    </form>
  );
}

function RemoveForm({ name, onDone }: { name: string; onDone(): void }) {
  const { api } = useStore();
  const { busy, error, submit } = useSubmit();
  return (
    <div className="flex flex-wrap items-center gap-2 px-3 pb-2 text-[12px]">
      <span className="text-text-2">Remove {name} and their devices from your tailnet?</span>
      <Button variant="primary" disabled={busy} onClick={() => void submit(() => api.friendRemove(name)).then((ok) => ok && onDone())}>
        Remove
      </Button>
      <Button variant="ghost" onClick={onDone}>
        Cancel
      </Button>
      {error && <p className="w-full text-bad">{error}</p>}
    </div>
  );
}

function OauthForm({ onSaved, onCancel }: { onSaved(): void; onCancel?(): void }) {
  const { api } = useStore();
  const [id, setId] = useState("");
  const [secret, setSecret] = useState("");
  const { busy, error, submit } = useSubmit();
  return (
    <form
      className="flex flex-col gap-2 px-3 pb-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(() => api.friendsOauth(id, secret)).then((ok) => ok && onSaved());
      }}
    >
      <p className="text-[12px] leading-5 text-text-2">
        The service needs a Tailscale OAuth client to make sign-in keys. In Tailscale's admin console, under Settings, Trust credentials, make one with write access to Keys, Auth Keys (not OAuth Keys) and Devices, Core, both with the tag{" "}
        <span className="font-mono text-text">tag:friend</span>.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <input aria-label="Client id" placeholder="Client id" className={`${fieldClass} !w-48`} value={id} onChange={(e) => setId(e.target.value)} />
        <input aria-label="Client secret" placeholder="Client secret" type="password" autoComplete="off" className={`${fieldClass} !w-64`} value={secret} onChange={(e) => setSecret(e.target.value)} />
        <Button type="submit" variant="primary" disabled={busy || !id.trim() || !secret.trim()}>
          Save client
        </Button>
        {onCancel && (
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
      {error && <p className="text-[12px] text-bad">{error}</p>}
    </form>
  );
}
