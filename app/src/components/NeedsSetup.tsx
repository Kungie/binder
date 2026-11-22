import type { ReactNode } from "react";
import { Panel } from "./Panel";
import { Button } from "./ui";

/** What a screen still needs, as numbered steps, with one action. */
export function NeedsSetup({ title, steps, action }: { title: string; steps: ReactNode[]; action?: { label: string; run(): void } }) {
  return (
    <Panel title={title} label={title} className="max-w-[640px]">
      <ol className="flex flex-col gap-2 border-t border-line p-3 text-[12px] leading-5 text-text-2">
        {steps.map((s, i) => (
          <li key={i} className="flex gap-3">
            <span className="flex size-5 shrink-0 items-center justify-center rounded-full border border-line-strong font-mono text-[11px] text-text-3">{i + 1}</span>
            <span className="min-w-0 flex-1">{s}</span>
          </li>
        ))}
      </ol>
      {action && (
        <div className="border-t border-line px-3 py-2">
          <Button onClick={action.run}>{action.label}</Button>
        </div>
      )}
    </Panel>
  );
}

export const Code = ({ children }: { children: ReactNode }) => <span className="font-mono text-text">{children}</span>;
