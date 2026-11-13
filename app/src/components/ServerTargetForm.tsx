import { useState } from "react";
import { useStore } from "../lib/store";
import { Button, Field, fieldClass } from "./ui";

/** Sets the home server's SSH target in the config. */
export function ServerTargetForm({ initial = "", onSaved }: { initial?: string; onSaved(): void }) {
  const { api } = useStore();
  const [target, setTarget] = useState(initial);
  const save = async () => {
    const cfg = await api.getConfig();
    await api.setConfig({ ...cfg, server: target.trim() || null });
    onSaved();
  };
  return (
    <Field label="SSH target">
      <div className="flex gap-2">
        <input
          aria-label="SSH target"
          className={`${fieldClass} font-mono`}
          placeholder="user@host or a Host from ~/.ssh/config"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void save()}
        />
        <Button onClick={() => void save()}>Connect</Button>
      </div>
    </Field>
  );
}
