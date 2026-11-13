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
  return { connected, path, recheck: () => setTick((t) => t + 1) };
}

export const vaultNotice = (path: string) => `Vault not connected. Connect to the vault share in Finder so ${path} exists.`;
