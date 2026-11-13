import { inTauri } from "./api";

/** A folder chosen in the system dialog, or null (cancelled, or not running in the app). */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
}

/** Files chosen in the system dialog, or none. */
export async function pickFiles(): Promise<string[]> {
  if (!inTauri) return [];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const chosen = await open({ multiple: true, directory: false });
  return Array.isArray(chosen) ? chosen : typeof chosen === "string" ? [chosen] : [];
}
