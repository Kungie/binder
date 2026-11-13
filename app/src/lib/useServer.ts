import { useCallback, useState } from "react";
import { useStore } from "./store";
import type { ServerStatus } from "./types";
import { usePoll } from "./usePoll";

/** The home server's last good read, and the error from the latest try if it failed. `target` is undefined until the config loads. */
export function useServer(ms: number) {
  const { api } = useStore();
  const [target, setTarget] = useState<string | null | undefined>(undefined);
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const cfg = await api.getConfig();
    setTarget(cfg.server);
    if (!cfg.server) return;
    try {
      setStatus(await api.serverStatus());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [api]);

  usePoll(refresh, ms);
  return { target, status, error, refresh };
}

export const clock = (at: number) => new Date(at * 1000).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" });

export const gb = (bytes: number) => (bytes / 1024 ** 3).toFixed(1);

export const percent = (part: number, whole: number) => (whole > 0 ? `${Math.round((part / whole) * 100)}%` : "None");
