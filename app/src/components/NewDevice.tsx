import { useState } from "react";
import { useStore } from "../lib/store";
import { DEVICE_KIND_LABEL, type DeviceKind } from "../lib/types";
import { Button, Field, fieldClass } from "./ui";

export function NewDevice({ onClose, onCreated }: { onClose(): void; onCreated(): void }) {
  const { api } = useStore();
  const [name, setName] = useState("");
  const [kind, setKind] = useState<DeviceKind>("other");
  const [address, setAddress] = useState("");
  const [port, setPort] = useState("");
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    if (!name.trim()) return setError("Give it a name.");
    if (!address.trim()) return setError("Give it an address.");
    const p = port.trim() ? Number(port) : undefined;
    if (p !== undefined && !(Number.isInteger(p) && p > 0 && p < 65536)) return setError("A port is a number from 1 to 65535.");
    try {
      await api.createDevice({ name, kind, address, port: p });
      onCreated();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[18vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-label="New device"
        className="fade-in flex w-[460px] flex-col gap-3 rounded-panel border border-line-strong bg-panel p-4 shadow-menu"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") return onClose();
          if (e.key === "Enter") {
            e.preventDefault();
            void submit();
          }
        }}
      >
        <Field label="Name">
          <input autoFocus className={fieldClass} value={name} onChange={(e) => (setName(e.target.value), setError(null))} placeholder="nas" />
        </Field>
        <Field label="Kind">
          <select className={fieldClass} value={kind} onChange={(e) => setKind(e.target.value as DeviceKind)}>
            {Object.entries(DEVICE_KIND_LABEL).map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Address">
          <input className={`${fieldClass} font-mono`} value={address} onChange={(e) => (setAddress(e.target.value), setError(null))} placeholder="10.0.7.40 or nas.local" />
        </Field>
        <Field label="Port">
          <input className={`${fieldClass} font-mono`} value={port} onChange={(e) => (setPort(e.target.value), setError(null))} placeholder="22" inputMode="numeric" />
        </Field>
        <div className="mt-1 flex items-center gap-2">
          {error && <span className="text-[12px] text-text-2">{error}</span>}
          <Button variant="primary" className="ml-auto" onClick={() => void submit()}>
            Create device
          </Button>
        </div>
      </div>
    </div>
  );
}
