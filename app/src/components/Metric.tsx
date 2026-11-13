import type { ReactNode } from "react";
import { Kpi } from "./panelui/kpi";

/** One number in a panel, PanelUI style. */
export function Metric({ title, value, detail }: { title: string; value: ReactNode; detail?: ReactNode }) {
  return (
    <Kpi surface role="group" aria-label={`${title}: ${value}`} className="!gap-1 !p-4">
      <Kpi.Header>
        <Kpi.Title className="!text-[12px] !font-normal">{title}</Kpi.Title>
      </Kpi.Header>
      <Kpi.Value className="font-mono !text-[24px] !font-normal">{value}</Kpi.Value>
      {detail !== undefined && <span className="font-mono text-[11px] text-text-3">{detail}</span>}
    </Kpi>
  );
}

/** A filled dot for on, a hollow one for off. State shows through shape, never colour. */
export function Dot({ on, label }: { on: boolean; label?: string }) {
  return (
    <span
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={`inline-block h-1.5 w-1.5 shrink-0 rounded-full ${on ? "bg-ok" : "border border-bad"}`}
    />
  );
}
