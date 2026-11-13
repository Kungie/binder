import { useCallback, useEffect, useRef, useState } from "react";
import { Dot } from "../components/Metric";
import { NewDevice } from "../components/NewDevice";
import { Button, Field, fieldClass } from "../components/ui";
import { useStore } from "../lib/store";
import { DEVICE_KIND_LABEL, type Device, type DevicePatch } from "../lib/types";
import { usePoll } from "../lib/usePoll";

export function Devices() {
  const { api } = useStore();
  const [devices, setDevices] = useState<Device[]>([]);
  const [online, setOnline] = useState<Record<number, boolean>>({});
  const [creating, setCreating] = useState(false);
  const [open, setOpen] = useState<number | null>(null);

  const load = useCallback(async () => {
    setDevices(await api.listDevices());
    setOnline(Object.fromEntries(await api.probeDevices()));
  }, [api]);
  usePoll(load, 30_000);

  const current = devices.find((d) => d.id === open);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Devices</h1>
        <span className="font-mono text-[11px] text-text-3">{devices.length}</span>
        <Button variant="primary" className="ml-auto" onClick={() => setCreating(true)}>
          New device
        </Button>
      </header>
      <div className="px-5 pb-5">
        <div className="rounded-panel border border-line bg-panel">
          <table aria-label="Devices" className="w-full table-fixed text-[12px]">
            <thead>
              <tr className="text-left text-[11px] text-text-3">
                <th className="py-2 pl-3 font-normal">Name</th>
                <th className="w-[110px] py-2 font-normal">Kind</th>
                <th className="py-2 font-normal">Address</th>
                <th className="w-[110px] py-2 pr-3 font-normal">Status</th>
              </tr>
            </thead>
            <tbody>
              {devices.map((d) => (
                <tr
                  key={d.id}
                  onClick={() => setOpen(d.id)}
                  className={`h-9 cursor-default border-t border-line ${open === d.id ? "bg-raised" : "hover:bg-raised/60"}`}
                >
                  <td className="truncate pl-3">
                    <button type="button" className="text-left text-text focus:outline-none" onClick={() => setOpen(d.id)}>
                      {d.name}
                    </button>
                  </td>
                  <td className="text-text-2">{DEVICE_KIND_LABEL[d.kind]}</td>
                  <td className="truncate font-mono text-[11px] text-text-2">
                    {d.address}
                    <span className="text-text-3">:{d.port}</span>
                  </td>
                  <td className="pr-3">
                    {d.id in online && (
                      <span className="flex items-center gap-2 text-text-2">
                        <Dot on={online[d.id]} />
                        {online[d.id] ? "Online" : "Offline"}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {devices.length === 0 && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">No devices yet. Add the Mac, the server and the printer.</p>}
        </div>
      </div>
      {creating && <NewDevice onClose={() => setCreating(false)} onCreated={() => void load()} />}
      {current && <DeviceSheet key={current.id} device={current} onClose={() => setOpen(null)} onChanged={() => void load()} />}
    </div>
  );
}

/** The side sheet for one device. Everything typed saves when it closes. */
function DeviceSheet({ device, onClose, onChanged }: { device: Device; onClose(): void; onChanged(): void }) {
  const { api } = useStore();
  const [form, setForm] = useState({ name: device.name, kind: device.kind, address: device.address, port: String(device.port), note: device.note });
  const [confirm, setConfirm] = useState(false);
  const latest = useRef(form);
  latest.current = form;

  const close = useRef(() => {});
  close.current = () => {
    const f = latest.current;
    const patch: DevicePatch = {};
    if (f.name.trim() && f.name !== device.name) patch.name = f.name;
    if (f.kind !== device.kind) patch.kind = f.kind;
    if (f.address.trim() && f.address !== device.address) patch.address = f.address;
    const port = Number(f.port);
    if (Number.isInteger(port) && port > 0 && port < 65536 && port !== device.port) patch.port = port;
    if (f.note !== device.note) patch.note = f.note;
    onClose();
    if (Object.keys(patch).length) void api.updateDevice(device.id, patch).then(onChanged);
  };

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => e.key === "Escape" && close.current();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const set = (k: keyof typeof form) => (e: { target: { value: string } }) => setForm((f) => ({ ...f, [k]: e.target.value }));

  return (
    <aside role="dialog" aria-label={device.name} className="fade-in absolute inset-y-0 right-0 z-30 flex w-[380px] flex-col border-l border-line bg-panel shadow-menu">
      <header className="flex h-11 items-center gap-2 border-b border-line px-4">
        <span className="text-[13px] font-medium">{device.name}</span>
        <Button variant="ghost" className="ml-auto" onClick={() => close.current()} aria-label="Close">
          Esc
        </Button>
      </header>
      <div className="flex flex-1 flex-col gap-3 overflow-y-auto p-4">
        <Field label="Name">
          <input className={fieldClass} value={form.name} onChange={set("name")} />
        </Field>
        <Field label="Kind">
          <select className={fieldClass} value={form.kind} onChange={set("kind")}>
            {Object.entries(DEVICE_KIND_LABEL).map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Address">
          <input className={`${fieldClass} font-mono`} value={form.address} onChange={set("address")} />
        </Field>
        <Field label="Port">
          <input className={`${fieldClass} font-mono`} value={form.port} onChange={set("port")} inputMode="numeric" />
        </Field>
        <label className="flex flex-1 flex-col gap-1.5 text-[12px]">
          <span className="text-text-3">Note</span>
          <textarea
            value={form.note}
            onChange={set("note")}
            placeholder="Where it is, what it runs"
            className="min-h-32 flex-1 resize-none rounded-control border border-line bg-bg p-2 font-mono text-[12px] leading-relaxed text-text placeholder:text-text-3 focus:border-text-3 focus:outline-none"
          />
        </label>
      </div>
      <footer className="flex items-center border-t border-line px-4 py-3">
        {confirm ? (
          <>
            <span className="text-[12px] text-text-2">Delete {device.name}?</span>
            <Button variant="ghost" className="ml-auto" onClick={() => setConfirm(false)}>
              Keep
            </Button>
            <Button onClick={() => void api.deleteDevice(device.id).then(() => (onClose(), onChanged()))}>Delete</Button>
          </>
        ) : (
          <Button variant="ghost" onClick={() => setConfirm(true)}>
            Delete device
          </Button>
        )}
      </footer>
    </aside>
  );
}
