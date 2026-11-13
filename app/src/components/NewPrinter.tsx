import { useState } from "react";
import { useStore } from "../lib/store";
import { Button, Field, Notice, fieldClass } from "./ui";

export function NewPrinter({ onClose, onAdded }: { onClose(): void; onAdded(): void }) {
  const { api } = useStore();
  const [form, setForm] = useState({ name: "", address: "", serial: "" });
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const set = (k: keyof typeof form) => (e: { target: { value: string } }) => (setForm((f) => ({ ...f, [k]: e.target.value })), setError(null));

  const submit = async () => {
    if (!form.name.trim() || !form.address.trim() || !form.serial.trim() || !code.trim()) return setError("Fill in all four.");
    setBusy(true);
    try {
      await api.addPrinter({ ...form, accessCode: code.trim() });
      setCode("");
      onAdded();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/60 pt-[18vh]" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-label="Add printer"
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
          <input autoFocus className={fieldClass} value={form.name} onChange={set("name")} placeholder="workshop" />
        </Field>
        <Field label="Address">
          <input className={`${fieldClass} font-mono`} value={form.address} onChange={set("address")} placeholder="10.0.7.30" />
        </Field>
        <Field label="Serial">
          <input className={`${fieldClass} font-mono uppercase`} value={form.serial} onChange={set("serial")} spellCheck={false} />
        </Field>
        <Field label="Access code">
          <input
            type="password"
            autoComplete="off"
            className={`${fieldClass} font-mono`}
            value={code}
            onChange={(e) => (setCode(e.target.value), setError(null))}
          />
        </Field>
        <Notice>Turn on LAN mode on the printer. The access code is in its network settings and goes straight to the Keychain.</Notice>
        <div className="mt-1 flex items-center gap-2">
          {error && <span className="text-[12px] text-text-2">{error}</span>}
          <Button variant="primary" className="ml-auto" disabled={busy} onClick={() => void submit()}>
            Add printer
          </Button>
        </div>
      </div>
    </div>
  );
}
