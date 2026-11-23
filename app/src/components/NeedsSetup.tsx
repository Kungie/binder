import { type ReactNode, useState } from "react";
import { GUIDES } from "../lib/guides";
import { Shot } from "./Guide";
import { Button } from "./ui";

/** What a screen still needs: what it does, numbered steps, one action, and a picture of the screen once it works. */
export function NeedsSetup({
  title,
  lead,
  steps,
  action,
  shot,
  children,
}: {
  title: string;
  lead?: string;
  steps: ReactNode[];
  action?: { label: string; run(): void };
  /** The screen whose picture shows what it looks like set up. */
  shot?: string;
  /** A form in place of the action, under the steps. */
  children?: ReactNode;
}) {
  const guide = shot ? GUIDES[shot] : undefined;
  return (
    <section aria-label={title} className="screen-in mx-auto flex w-full max-w-[760px] flex-col gap-6 py-6">
      <div className="flex flex-col gap-1.5">
        <span className="text-[11px] tracking-[0.04em] text-text-3 uppercase">Set up</span>
        <h2 className="text-[20px] leading-7 font-medium tracking-[-0.01em] text-text">{title}</h2>
        {lead && <p className="max-w-[600px] text-[13px] leading-5 text-text-2">{lead}</p>}
      </div>
      <ol className="flex flex-col">
        {steps.map((s, i) => (
          <li key={i} className="relative flex gap-4 pb-5 last:pb-0">
            {i < steps.length - 1 && <span aria-hidden className="absolute top-6 bottom-0 left-[11px] w-px bg-line" />}
            <span className="relative flex size-6 shrink-0 items-center justify-center rounded-full border border-line-strong bg-panel font-mono text-[11px] text-text">{i + 1}</span>
            <div className="min-w-0 flex-1 pt-0.5 text-[13px] leading-5 text-text-2">{s}</div>
          </li>
        ))}
      </ol>
      {children}
      {action && (
        <div>
          <Button variant="primary" className="h-8 px-3.5" onClick={action.run}>
            {action.label}
          </Button>
        </div>
      )}
      {guide && (
        <div className="flex flex-col gap-2 pt-2">
          <span className="text-[12px] text-text-3">Once it's set up</span>
          <div className="relative">
            <Shot src={guide.shot} title={guide.title} className="opacity-70" />
            <div aria-hidden className="pointer-events-none absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-bg to-transparent" />
          </div>
        </div>
      )}
    </section>
  );
}

export const Code = ({ children }: { children: ReactNode }) => <span className="rounded-[4px] bg-card px-1 font-mono text-[12px] text-text">{children}</span>;

/** A command to run, with Copy. */
export function Cmd({ children }: { children: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="mt-2 flex items-center gap-2 rounded-control border border-line bg-card py-1.5 pr-1.5 pl-3">
      <code className="min-w-0 flex-1 overflow-x-auto font-mono text-[12px] whitespace-pre text-text">{children}</code>
      <Button
        variant="ghost"
        aria-label={`Copy ${children}`}
        onClick={() =>
          void navigator.clipboard?.writeText(children).then(
            () => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            },
            () => {},
          )
        }
      >
        {copied ? "Copied" : "Copy"}
      </Button>
    </div>
  );
}
