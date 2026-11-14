import { useEffect, useState } from "react";
import { bytes } from "../lib/format";
import { useStore } from "../lib/store";
import type { Space } from "../lib/types";
import { Progress } from "./Progress";

/**
 * The SD card's space, when the share has an SD card of its own: only the fast path can tell its disk apart. An SD
 * folder with no card in it reports the SSD's own space, so a space equal to the share's is no card.
 */
export function useSdSpace(mounted: boolean): Space | null {
  const { api } = useStore();
  const [sd, setSd] = useState<Space | null>(null);
  useEffect(() => {
    let live = true;
    if (!mounted) return setSd(null);
    void (async () => {
      const fast = await api.cloudFastPath().catch(() => "smb");
      const root = fast === "ssh" ? await api.cloudList("", false).catch(() => []) : [];
      const space = root.some((e) => e.rel === "SD" && e.kind === "dir") ? await api.cloudSpace("SD").catch(() => null) : null;
      const ssd = space ? await api.cloudSpace("").catch(() => null) : null;
      const same = space && ssd && space.total === ssd.total && space.free === ssd.free;
      if (live) setSd(same ? null : space);
    })();
    return () => {
      live = false;
    };
  }, [api, mounted]);
  return sd;
}

function Bar({ label, space }: { label: string; space: Space }) {
  const used = space.total ? ((space.total - space.free) / space.total) * 100 : 0;
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <span className="text-[11px] text-text-2">{label}</span>
        <span className="ml-auto font-mono text-[11px] text-text-3">{`${bytes(space.free)} free of ${bytes(space.total)}`}</span>
      </div>
      <Progress value={used} label={`${label} space used`} />
    </div>
  );
}

/** The SSD and the SD card: stacked in a panel, or side by side (`row`) where height is short, as in a header. */
export function DiskBars({ ssd, sd, row = false }: { ssd: Space | null; sd: Space | null; row?: boolean }) {
  if (!ssd && !sd) return null;
  return (
    <div role="group" aria-label="Disk space" className={row ? "flex flex-row gap-5 [&>*]:w-52" : "flex flex-col gap-2"}>
      {ssd && <Bar label="SSD" space={ssd} />}
      {sd && <Bar label="SD card" space={sd} />}
    </div>
  );
}
