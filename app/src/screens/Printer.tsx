import { useState } from "react";
import { Dot } from "../components/Metric";
import { NewPrinter } from "../components/NewPrinter";
import { Panel } from "../components/Panel";
import { Progress } from "../components/Progress";
import { Button, Notice } from "../components/ui";
import { useStore } from "../lib/store";
import type { PrinterCommand, PrinterView } from "../lib/types";
import { STATE_LABEL, minutes, temp, usePrinters } from "../lib/usePrinters";

export function Printer() {
  const { printers, reload } = usePrinters();
  const [adding, setAdding] = useState(false);
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Printer</h1>
        <Button variant="primary" className="ml-auto" onClick={() => setAdding(true)}>
          Add printer
        </Button>
      </header>
      <div className="flex max-w-[960px] flex-col gap-3 px-5 pb-5">
        {printers.map((p) => (
          <PrinterPanel key={p.config.serial} view={p} onRemoved={() => void reload()} />
        ))}
        {printers.length === 0 && (
          <Notice>No printer yet. Add a Bambu printer in LAN mode with its address, serial number and access code.</Notice>
        )}
      </div>
      {adding && <NewPrinter onClose={() => setAdding(false)} onAdded={() => void reload()} />}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[11px] text-text-3">{label}</span>
      <span className="font-mono text-[12px] text-text">{value}</span>
    </div>
  );
}

function PrinterPanel({ view, onRemoved }: { view: PrinterView; onRemoved(): void }) {
  const { api } = useStore();
  const { config, state, online, error } = view;
  const [confirm, setConfirm] = useState<"stop" | "remove" | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const send = (kind: PrinterCommand) => {
    setConfirm(null);
    api.printerCommand(config.serial, kind).catch((e) => setFailed(e instanceof Error ? e.message : String(e)));
  };
  const s = state.gcode_state;
  const live = s === "RUNNING" || s === "PAUSE";

  return (
    <Panel
      label={config.name}
      title={config.name}
      action={
        <span className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-text-3">{config.address}</span>
          <Dot on={online} />
          <span>{online ? "Online" : "Offline"}</span>
        </span>
      }
    >
      <div className="flex flex-col gap-4 border-t border-line p-4">
        {error && <Notice>{error}</Notice>}
        <div className="flex items-baseline gap-3">
          <span className="min-w-0 truncate text-[15px] font-medium">{state.job ?? "No job"}</span>
          <span className="text-[12px] text-text-3">{s ? (STATE_LABEL[s] ?? s) : ""}</span>
          <span className="ml-auto font-mono text-[12px] text-text">{state.percent === null ? "" : `${state.percent}%`}</span>
        </div>
        <Progress value={state.percent ?? 0} label="Progress" />
        <div className="grid grid-cols-4 gap-3">
          <Stat label="Layer" value={state.layer === null ? "None" : `${state.layer} of ${state.total_layers ?? "?"}`} />
          <Stat label="Time left" value={minutes(state.remaining_min)} />
          <Stat label="Nozzle" value={temp(state.nozzle_c, state.nozzle_target_c)} />
          <Stat label="Bed" value={temp(state.bed_c, state.bed_target_c)} />
        </div>
        {failed && <Notice>{failed}</Notice>}
        <div className="flex items-center gap-2">
          {confirm === "stop" ? (
            <>
              <span className="text-[12px] text-text-2">Stop the print? It can't be resumed.</span>
              <Button variant="ghost" className="ml-auto" onClick={() => setConfirm(null)}>
                Keep printing
              </Button>
              <Button onClick={() => send("stop")}>Stop print</Button>
            </>
          ) : confirm === "remove" ? (
            <>
              <span className="text-[12px] text-text-2">Remove {config.name}? Its access code is deleted from the Keychain.</span>
              <Button variant="ghost" className="ml-auto" onClick={() => setConfirm(null)}>
                Keep
              </Button>
              <Button onClick={() => void api.removePrinter(config.serial).then(onRemoved)}>Remove</Button>
            </>
          ) : (
            <>
              {s === "RUNNING" && <Button onClick={() => send("pause")}>Pause</Button>}
              {s === "PAUSE" && <Button onClick={() => send("resume")}>Resume</Button>}
              {live && <Button onClick={() => setConfirm("stop")}>Stop</Button>}
              <Button variant="ghost" className="ml-auto" onClick={() => setConfirm("remove")}>
                Remove printer
              </Button>
            </>
          )}
        </div>
      </div>
    </Panel>
  );
}
