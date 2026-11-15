import { useEffect, useState } from "react";
import { useStore } from "./store";

/** Whether the vault folder is there, and where it should be. Checked again when asked. */
export function useVault() {
  const { api } = useStore();
  const [connected, setConnected] = useState<boolean | null>(null);
  const [path, setPath] = useState("/Volumes/vault");
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    void Promise.all([api.vaultConnected(), api.getConfig()]).then(([c, cfg]) => {
      if (!live) return;
      setConnected(c);
      setPath(cfg.vault);
    });
    return () => {
      live = false;
    };
  }, [api, tick]);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Mounts the vault share now, with the password saved in the Keychain. */
  const connect = async () => {
    setConnecting(true);
    setError(null);
    try {
      setConnected(await api.vaultConnect());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(false);
    }
  };
  return { connected, path, recheck: () => setTick((t) => t + 1), connect, connecting, error };
}

export const vaultNotice = (path: string) => `Vault not connected. Connect to the vault share in Finder so ${path} exists.`;
