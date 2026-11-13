import { useCallback, useEffect, useState } from "react";
import { useStore } from "./store";
import type { PrinterView } from "./types";

/** Every configured printer, kept current by the app's printer events. */
export function usePrinters() {
  const { api } = useStore();
  const [printers, setPrinters] = useState<PrinterView[]>([]);
  const reload = useCallback(async () => setPrinters(await api.printers()), [api]);
  useEffect(() => {
    void reload();
    return api.onPrinter((v) => setPrinters((list) => list.map((p) => (p.config.serial === v.config.serial ? v : p))));
  }, [api, reload]);
  return { printers, reload };
}

export const STATE_LABEL: Record<string, string> = {
  RUNNING: "Printing",
  PAUSE: "Paused",
  PREPARE: "Preparing",
  FINISH: "Finished",
  FAILED: "Failed",
  IDLE: "Idle",
};

export const minutes = (m: number | null) => (m === null ? "None" : m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`);

export const temp = (now: number | null, target: number | null) => (now === null ? "None" : `${Math.round(now)} / ${Math.round(target ?? 0)}°C`);
