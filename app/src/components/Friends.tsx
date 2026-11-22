import { useCallback, useEffect, useState } from "react";
import { useStore } from "../lib/store";
import type { Friend, FriendsService } from "../lib/types";
import { Panel } from "./Panel";
import { Button, fieldClass } from "./ui";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const day = (t: number) => new Date(t * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });

type Open = { kind: "add" } | { kind: "oauth" } | { kind: "password"; name: string } | { kind: "remove"; name: string } | null;

/** Friends who sign in to Binder VPN with a user name and a password, kept by the service on the server. */
export function Friends() {
  const { api } = useStore();
  const [service, setService] = useState<FriendsService | null>(null);
  const [list, setList] = useState<Friend[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<Open>(null);
  const [note, setNote] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
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

  return (
    <Panel
      title="Friends"
      label="Friends"
      action={
        service?.configured && (
          <div className="flex items-center gap-1">
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
        <p className="px-3 py-2 text-[12px] leading-5 text-text-3">They sign in to Binder VPN with a user name and a password and reach the internet through your exit nodes, nothing else.</p>
        {error && <p className="px-3 pb-2 text-[12px] text-bad">{error}</p>}
        {service && !service.installed && (
          <p className="px-3 pb-3 text-[12px] leading-5 text-text-2">
            Binder VPN's sign-in service isn't running on the server. It is the <span className="font-mono text-text">vpn</span> service of <span className="font-mono text-text">~/sites/binder-site</span>.
          </p>
        )}
        {service?.installed && !service.configured && <OauthForm onSaved={() => void done(null)} />}
        {service?.configured && open?.kind === "oauth" && <OauthForm onSaved={() => void done("Client saved.")} onCancel={() => setOpen(null)} />}
        {note && <p className="px-3 pb-2 text-[12px] text-ok">{note}</p>}
        {open?.kind === "add" && <FriendForm onDone={() => void done(null)} />}
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
            {open?.kind === "password" && open.name === f.name && <PasswordForm name={f.name} onDone={(ok) => void done(ok ? `Password changed for ${f.name}.` : null)} />}
            {open?.kind === "remove" && open.name === f.name && <RemoveForm name={f.name} onDone={() => void done(null)} />}
          </div>
        ))}
      </div>
    </Panel>
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

function FriendForm({ onDone }: { onDone(name: string | null): void }) {
  const { api } = useStore();
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const { busy, error, submit } = useSubmit();
  return (
    <form
      className="flex flex-wrap items-center gap-2 border-t border-line px-3 py-2"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(() => api.friendAdd(name.trim(), password)).then((ok) => ok && onDone(name.trim()));
      }}
    >
      <input aria-label="User name" placeholder="User name" autoFocus className={`${fieldClass} !w-40`} value={name} onChange={(e) => setName(e.target.value.toLowerCase())} />
      <input aria-label="Password" placeholder="Password, 8 or more" type="password" autoComplete="new-password" className={`${fieldClass} !w-48`} value={password} onChange={(e) => setPassword(e.target.value)} />
      <Button type="submit" variant="primary" disabled={busy || !name.trim() || !password}>
        Add
      </Button>
      <Button variant="ghost" onClick={() => onDone(null)}>
        Cancel
      </Button>
      {error && <p className="w-full text-[12px] text-bad">{error}</p>}
    </form>
  );
}

function PasswordForm({ name, onDone }: { name: string; onDone(ok: boolean): void }) {
  const { api } = useStore();
  const [password, setPassword] = useState("");
  const { busy, error, submit } = useSubmit();
  return (
    <form
      className="flex flex-wrap items-center gap-2 px-3 pb-2"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(() => api.friendPassword(name, password)).then((ok) => ok && onDone(true));
      }}
    >
      <input aria-label="New password" placeholder="New password" type="password" autoFocus autoComplete="new-password" className={`${fieldClass} !w-48`} value={password} onChange={(e) => setPassword(e.target.value)} />
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
