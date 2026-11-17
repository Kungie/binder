import { useEffect, useState } from "react";
import { Dot } from "../components/Metric";
import { NewPrinter } from "../components/NewPrinter";
import { Panel } from "../components/Panel";
import { Progress } from "../components/Progress";
import { Button, Notice, Switch, fieldClass } from "../components/ui";
import { useStore } from "../lib/store";
import type { PrintRecord, PrinterCommand, PrinterView } from "../lib/types";
import { ago } from "../lib/time";
import { STATE_LABEL, minutes, temp, usePrinters } from "../lib/usePrinters";
import { Empty } from "../components/Mascot";

export function Printer() {
  const { printers, reload } = usePrinters();
  const [adding, setAdding] = useState(false);
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">3D Printer</h1>
        <Button variant="primary" className="ml-auto" onClick={() => setAdding(true)}>
          Add printer
        </Button>
      </header>
      <div className="flex max-w-[960px] flex-col gap-3 px-5 pb-5">
        {printers.map((p) => (
          <PrinterPanel key={p.config.serial} view={p} onRemoved={() => void reload()} />
        ))}
        {printers.length > 0 && <History />}
        {printers.length === 0 && (
          <div className="rounded-panel border border-line bg-panel">
            <Empty>No printer yet. Add a Bambu printer in LAN mode with its address, serial number and access code.</Empty>
          </div>
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
  const [camera, setCamera] = useState(false);
  const problems = view.problems ?? [];
  const fan = state.fan_percent ?? null;
  const fanStep = fan === null ? "" : String(FAN_STEPS.reduce((a, b) => (Math.abs(b - fan) < Math.abs(a - fan) ? b : a)));

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
        {problems.length > 0 && (
          <ul aria-label="Printer messages" className="flex flex-col gap-1.5 rounded-control border border-line bg-card p-3">
            {problems.map((p) => (
              <li key={p.code} className="flex flex-col gap-0.5 text-[12px]">
                <span className="flex items-start gap-2 text-text">
                  <span className="mt-1 size-1.5 shrink-0 rounded-full bg-urgent" aria-hidden="true" />
                  {p.text}
                </span>
                <span className="pl-3.5 font-mono text-[11px] text-text-3">{p.code.match(/.{1,4}/g)?.join("-")}</span>
              </li>
            ))}
          </ul>
        )}
        {camera && <Camera serial={config.serial} name={config.name} />}
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
        {online && (
          <div className="flex flex-wrap items-center gap-4 text-[12px] text-text-2">
            {state.light_on !== undefined && state.light_on !== null && (
              <span className="flex items-center gap-2">
                <Switch label="Light" checked={state.light_on} onChange={(on) => send({ light: { on } })} />
                Light
              </span>
            )}
            {fan !== null && (
              <label className="flex items-center gap-2 whitespace-nowrap">
                Part fan
                <select aria-label="Part fan" className={`${fieldClass} h-7 w-24`} value={fanStep} onChange={(e) => send({ fan: { percent: Number(e.target.value) } })}>
                  {FAN_STEPS.map((f) => (
                    <option key={f} value={f}>
                      {f === 0 ? "Off" : `${f}%`}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <Button variant="ghost" className="ml-auto" onClick={() => setCamera((c) => !c)}>
              {camera ? "Hide camera" : "Show camera"}
            </Button>
          </div>
        )}
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

const FAN_STEPS = [0, 25, 50, 75, 100];

/** The printer's camera, one still every few seconds on an A1, only while shown and the window is visible. */
function Camera({ serial, name }: { serial: string; name: string }) {
  const { api } = useStore();
  const [src, setSrc] = useState<string | null>(null);
  const [visible, setVisible] = useState(!document.hidden);
  useEffect(() => {
    const on = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", on);
    return () => document.removeEventListener("visibilitychange", on);
  }, []);
  useEffect(() => {
    if (!visible) return;
    let last: string | null = null;
    const stop = api.printerCamera(serial, (jpeg) => {
      const url = URL.createObjectURL(new Blob([jpeg as BlobPart], { type: "image/jpeg" }));
      setSrc(url);
      if (last) URL.revokeObjectURL(last);
      last = url;
    });
    return () => {
      stop();
      if (last) URL.revokeObjectURL(last);
    };
  }, [api, serial, visible]);
  return (
    <div className="relative aspect-video overflow-hidden rounded-control border border-line bg-card">
      {src ? (
        <img src={src} alt={`${name} camera`} className="size-full object-contain" />
      ) : (
        <span className="absolute inset-0 flex items-center justify-center text-[12px] text-text-3">Waiting for the camera…</span>
      )}
    </div>
  );
}

const length = (m: number) => (m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`);

/** Prints that ended, newest first. */
function History() {
  const { api } = useStore();
  const [list, setList] = useState<PrintRecord[] | null>(null);
  useEffect(() => {
    const load = () => void api.printerHistory().then(setList, () => setList([]));
    load();
    return api.onPrintEnded(load);
  }, [api]);
  return (
    <Panel label="Print history" title="Print history" action={list && <span className="font-mono text-[11px] text-text-3">{list.length}</span>}>
      {list && list.length === 0 ? (
        <Empty>No prints yet. Each one shows here when it ends.</Empty>
      ) : (
        <ul className="border-t border-line">
          {(list ?? []).map((p, i) => (
            <li key={`${p.at}-${i}`} className="flex items-center gap-3 border-b border-line px-3 py-2 text-[12px] last:border-b-0">
              <span className={`size-1.5 shrink-0 rounded-full ${p.ok ? "bg-ok" : "bg-bad"}`} aria-hidden="true" />
              <span className="min-w-0 flex-1 truncate text-text">{p.job}</span>
              <span className="shrink-0 text-text-2">{p.ok ? (p.minutes !== null ? length(p.minutes) : "Done") : p.layer !== null ? `Failed at layer ${p.layer}` : "Failed"}</span>
              <span className="w-20 shrink-0 text-right text-[11px] text-text-3">{ago(p.at)}</span>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
