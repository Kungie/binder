import { useCallback, useEffect, useState } from "react";
import { previewStep } from "../lib/api";
import { useStore } from "../lib/store";
import type { SetupChecks } from "../lib/types";
import { createPortal } from "react-dom";
import { GUIDES } from "../lib/guides";
import { Shot } from "./Guide";
import { Mascot, MascotMark } from "./Mascot";
import { Button, fieldClass } from "./ui";

/** Settings sends this to open the setup again. */
export const SETUP_EVENT = "binder:setup";

const STEPS = ["Welcome", "Home server", "Vault", "GitHub", "Done"] as const;

/** The first-run setup: what Binder is, the home server, the vault and gh, each skippable. Shown once, while no
 *  server is set, and again from Settings. */
export function Welcome() {
  const { api } = useStore();
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(previewStep);
  const [checks, setChecks] = useState<SetupChecks | null>(null);

  useEffect(() => {
    let live = true;
    void api.getConfig().then((c) => live && setOpen(!c.onboarded && !c.server), () => {});
    const again = () => {
      setStep(0);
      setOpen(true);
    };
    window.addEventListener(SETUP_EVENT, again);
    return () => {
      live = false;
      window.removeEventListener(SETUP_EVENT, again);
    };
  }, [api]);

  const check = useCallback(() => void api.setupChecks().then(setChecks, () => {}), [api]);
  useEffect(() => {
    if (open) check();
  }, [open, check]);

  const finish = async () => {
    setOpen(false);
    const c = await api.getConfig();
    await api.setConfig({ ...c, onboarded: true });
  };

  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && void finish();
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  if (!open) return null;
  const next = () => setStep((s) => Math.min(s + 1, STEPS.length - 1));
  const back = () => setStep((s) => Math.max(s - 1, 0));
  const shot = SHOTS[step];
  return createPortal(
    <div role="dialog" aria-modal="true" aria-label="Set up Binder" className="guide-in fixed inset-0 z-50 flex bg-bg">
      <div className="flex w-[440px] shrink-0 flex-col border-r border-line bg-panel">
        <div className="flex items-center gap-2 px-8 pt-8 text-[13px] text-text">
          <MascotMark size={18} />
          <span className="font-medium">Binder</span>
          <span className="text-text-3">Setup</span>
        </div>
        <ol aria-label="Steps" className="flex flex-col gap-0.5 px-6 pt-8">
          {STEPS.map((s, i) => (
            <li
              key={s}
              aria-current={i === step ? "step" : undefined}
              className={`flex items-center gap-3 rounded-control px-2 py-1.5 text-[13px] ${i === step ? "bg-raised text-text" : i < step ? "text-text-2" : "text-text-3"}`}
            >
              <span className={`flex size-5 items-center justify-center rounded-full border font-mono text-[10px] ${i < step ? "border-text-2 bg-text-2 text-bg" : i === step ? "border-text text-text" : "border-line-strong"}`}>
                {i < step ? (
                  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden>
                    <path d="M2 5.2l2 2 4-4.4" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                ) : (
                  i + 1
                )}
              </span>
              {s}
            </li>
          ))}
        </ol>
        <div className="mx-8 mt-8 border-t border-line" />
        <div key={step} className="screen-in flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-8 py-8 text-[13px] leading-5 text-text-2">
          {step === 0 && <Intro checks={checks} />}
          {step === 1 && <ServerStep onDone={next} />}
          {step === 2 && <VaultStep onDone={next} />}
          {step === 3 && <GithubStep checks={checks} onCheck={check} />}
          {step === 4 && (
            <>
              <h2 className="text-[22px] leading-7 font-medium tracking-[-0.01em] text-text">You're set</h2>
              <p className="text-[14px] leading-6">Each screen opens with a short guide the first time; press ? on any screen to see it again. Where something is still missing, the screen shows the steps.</p>
              <p className="text-text-3">Tasks, Projects and the menu bar work without anything else. Settings has everything you skipped, and Run setup again.</p>
            </>
          )}
        </div>
        <div className="flex items-center gap-2 border-t border-line px-8 py-4">
          {step < STEPS.length - 1 && (
            <Button variant="ghost" onClick={() => void finish()}>
              Skip setup
            </Button>
          )}
          <span className="flex-1" />
          {step > 0 && (
            <Button variant="ghost" onClick={back}>
              Back
            </Button>
          )}
          {step > 0 && step < 3 && (
            <Button variant="ghost" onClick={next}>
              Skip this step
            </Button>
          )}
          {step === 0 && (
            <Button variant="primary" className="h-8 px-3.5" onClick={next}>
              Start
            </Button>
          )}
          {step === 3 && (
            <Button variant="primary" className="h-8 px-3.5" onClick={next}>
              Next
            </Button>
          )}
          {step === STEPS.length - 1 && (
            <Button variant="primary" className="h-8 px-3.5" onClick={() => void finish()}>
              Start using Binder
            </Button>
          )}
        </div>
      </div>
      <div className="relative flex min-w-0 flex-1 flex-col justify-center gap-5 overflow-hidden px-14 py-12">
        <div aria-hidden className="pointer-events-none absolute inset-0 bg-[radial-gradient(60%_50%_at_60%_45%,rgba(255,255,255,0.05),transparent)]" />
        <div className="relative flex flex-col gap-1">
          <span className="text-[12px] text-text-3">{shot.eyebrow}</span>
          <span className="text-[15px] text-text">{shot.caption}</span>
        </div>
        <Shot key={shot.src} src={shot.src} title={shot.title} className="screen-in relative" />
      </div>
    </div>,
    document.body,
  );
}

/** What the picture beside each step shows. */
const SHOTS: { src: string; title: string; eyebrow: string; caption: string }[] = [
  { src: GUIDES.overview.shot, title: "Overview", eyebrow: "Overview", caption: "Your tasks, your server, Cloud and the printer in one window." },
  { src: GUIDES.server.shot, title: "Servers", eyebrow: "With a home server", caption: "Containers, services, sites and agents, read over SSH." },
  { src: GUIDES.projects.shot, title: "Projects", eyebrow: "With a vault", caption: "Each project keeps a note and daily lines in Obsidian." },
  { src: GUIDES.github.shot, title: "GitHub", eyebrow: "With gh", caption: "Pull requests and issues of your projects, answered from Binder." },
  { src: GUIDES.tasks.shot, title: "Tasks", eyebrow: "Ready", caption: "Start with a task: press N anywhere in Binder." },
];

function Row({ name, needs, ok }: { name: string; needs: string; ok?: boolean }) {
  return (
    <li className="flex items-center gap-3 border-t border-line py-1.5 first:border-t-0">
      <span className="w-24 shrink-0 text-text">{name}</span>
      <span className="min-w-0 flex-1 text-[12px] text-text-3">{needs}</span>
      {ok !== undefined && <span className={`text-[11px] ${ok ? "text-ok" : "text-text-3"}`}>{ok ? "Ready" : "Not yet"}</span>}
    </li>
  );
}

function Intro({ checks }: { checks: SetupChecks | null }) {
  return (
    <>
      <div className="flex items-center gap-3">
        <Mascot size={36} />
        <h2 className="text-[22px] leading-7 font-medium tracking-[-0.01em] text-text">Welcome to Binder</h2>
      </div>
      <p>One window for the things you keep checking: tasks, projects, your home server and what runs on it, your devices and your printer. Each part needs one thing, and every step can wait.</p>
      <ul className="flex flex-col">
        <Row name="Servers" needs="SSH to a Linux server with your keys" />
        <Row name="Projects" needs="An Obsidian vault folder, and gh for GitHub" ok={checks ? checks.gh_signed_in : undefined} />
        <Row name="Devices, VPN" needs="Tailscale on this Mac" ok={checks ? checks.tailscale : undefined} />
        <Row name="Cloud" needs="A folder on the server shared over SMB" />
        <Row name="Printer" needs="A Bambu printer in LAN mode" />
      </ul>
    </>
  );
}

function ServerStep({ onDone }: { onDone(): void }) {
  const { api } = useStore();
  const [target, setTarget] = useState("");
  const [state, setState] = useState<"checking" | "up" | "down" | null>(null);
  const save = async () => {
    const t = target.trim();
    if (!t) return;
    setState("checking");
    const up = await api.serverReachable(t).catch(() => false);
    setState(up ? "up" : "down");
    if (!up) return;
    const c = await api.getConfig();
    await api.setConfig({ ...c, server: t, servers: [...new Set([...(c.servers ?? []), t])] });
    onDone();
  };
  return (
    <>
      <h2 className="text-[22px] leading-7 font-medium tracking-[-0.01em] text-text">Your home server</h2>
      <p>
        Binder reaches it with the system's <span className="font-mono text-text">ssh</span> and your keys, never a password. Use a Host from <span className="font-mono text-text">~/.ssh/config</span> or{" "}
        <span className="font-mono text-text">user@address</span>. It should already connect from Terminal without a prompt.
      </p>
      <div className="flex gap-2">
        <input
          aria-label="SSH target"
          autoFocus
          className={`${fieldClass} font-mono`}
          placeholder="user@host"
          value={target}
          onChange={(e) => {
            setTarget(e.target.value);
            setState(null);
          }}
          onKeyDown={(e) => e.key === "Enter" && void save()}
        />
        <Button variant="primary" disabled={!target.trim() || state === "checking"} onClick={() => void save()}>
          {state === "checking" ? "Connecting" : "Connect"}
        </Button>
      </div>
      {state === "down" && <p className="text-[12px] text-bad">Couldn't connect. Check that ssh {target.trim()} works in Terminal without a password, then try again.</p>}
      <p className="text-[12px] text-text-3">Servers, Sites, Agents and Cloud's fast path use it. Docker on the server shows containers; tmux runs the agents.</p>
    </>
  );
}

function VaultStep({ onDone }: { onDone(): void }) {
  const { api } = useStore();
  const [path, setPath] = useState("/Volumes/vault");
  const save = async () => {
    const c = await api.getConfig();
    await api.setConfig({ ...c, vault: path.trim() || "/Volumes/vault" });
    onDone();
  };
  return (
    <>
      <h2 className="text-[22px] leading-7 font-medium tracking-[-0.01em] text-text">Your Obsidian vault</h2>
      <p>Each project gets a note and daily lines in it, under Projects/. Any folder works: a vault on this Mac, or one on a share you mount.</p>
      <div className="flex gap-2">
        <input aria-label="Vault folder" className={`${fieldClass} font-mono`} value={path} onChange={(e) => setPath(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void save()} />
        <Button variant="primary" onClick={() => void save()}>
          Save
        </Button>
      </div>
      <p className="text-[12px] text-text-3">Nothing is written while the folder isn't there; lines wait and land later.</p>
    </>
  );
}

function GithubStep({ checks, onCheck }: { checks: SetupChecks | null; onCheck(): void }) {
  return (
    <>
      <h2 className="text-[22px] leading-7 font-medium tracking-[-0.01em] text-text">GitHub</h2>
      <p>Projects and the GitHub screen use GitHub's own command line with its login. Binder never sees a token.</p>
      <pre className="rounded-control border border-line bg-card p-2 font-mono text-[12px] text-text">{"brew install gh\ngh auth login"}</pre>
      <div className="flex items-center gap-3">
        <Button onClick={onCheck}>Check again</Button>
        {checks && <span className={`text-[12px] ${checks.gh_signed_in ? "text-ok" : "text-text-3"}`}>{checks.gh_signed_in ? "gh is signed in" : checks.gh ? "gh is installed, not signed in" : "gh isn't installed"}</span>}
      </div>
    </>
  );
}
