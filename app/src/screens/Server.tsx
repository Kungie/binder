import { Dot, Metric } from "../components/Metric";
import { Panel } from "../components/Panel";
import { ServerTargetForm } from "../components/ServerTargetForm";
import { Button, Notice } from "../components/ui";
import { clock, gb, percent, useServer } from "../lib/useServer";

export function Server() {
  const { target, status, error, refresh } = useServer(30_000);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <header className="flex h-12 shrink-0 items-center gap-3 px-5" data-tauri-drag-region>
        <h1 className="text-[13px] font-medium">Server</h1>
        {target && <span className="font-mono text-[11px] text-text-3">{target}</span>}
        {status && !error && <span className="text-[12px] text-text-3">Read {clock(status.at)}</span>}
        {target && (
          <Button variant="ghost" className="ml-auto" onClick={() => void refresh()}>
            Refresh
          </Button>
        )}
      </header>
      {target === null && (
        <div className="flex max-w-[960px] flex-col gap-3 px-5">
          <Panel title="Connect the home server">
            <div className="flex flex-col gap-3 border-t border-line p-3">
              <ServerTargetForm onSaved={() => void refresh()} />
              <Notice>Binder runs ssh with your keys and only reads: docker ps, systemctl and /proc.</Notice>
            </div>
          </Panel>
        </div>
      )}
      {target && (
        <div className="flex flex-col gap-3 px-5 pb-5">
          {error && (
            <div className="flex flex-col gap-0.5">
              <Notice>
                Can't reach {target}.{status ? ` Last read ${clock(status.at)}.` : ""}
              </Notice>
              {!error.startsWith("Can't reach") && <span className="font-mono text-[11px] text-text-3">{error}</span>}
            </div>
          )}
          {status && (
            <>
              <div className={`grid grid-cols-4 gap-3 ${error ? "opacity-60" : ""}`}>
                <Metric title="Load" value={status.load[0].toFixed(2)} detail={`${status.load[1].toFixed(2)}  ${status.load[2].toFixed(2)}`} />
                <Metric
                  title="Memory"
                  value={percent(status.mem_total - status.mem_available, status.mem_total)}
                  detail={`${gb(status.mem_total - status.mem_available)} of ${gb(status.mem_total)} GB`}
                />
                <Metric title="Disk" value={percent(status.disk_used, status.disk_size)} detail={`${gb(status.disk_used)} of ${gb(status.disk_size)} GB`} />
                <Metric title="Temperature" value={status.temp_c === null ? "None" : `${status.temp_c.toFixed(0)}°C`} />
              </div>
              <div className={`grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] items-start gap-3 ${error ? "opacity-60" : ""}`}>
                <Containers status={status} />
                <Panel title="Services" action={<span className="font-mono text-[11px] text-text-3">{status.services.length}</span>}>
                  <ul aria-label="Running services" className="border-t border-line">
                    {status.services.map((s) => (
                      <li key={s.name} className="flex flex-col gap-0.5 border-b border-line px-3 py-2 text-[12px] last:border-b-0">
                        <span className="font-mono text-[11px] text-text">{s.name}</span>
                        <span className="truncate text-text-3">{s.description}</span>
                      </li>
                    ))}
                    {status.services.length === 0 && <li className="px-3 py-3 text-[12px] text-text-3">No services read.</li>}
                  </ul>
                </Panel>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function Containers({ status }: { status: NonNullable<ReturnType<typeof useServer>["status"]> }) {
  const rows = [...status.containers].sort((a, b) => Number(b.state === "running") - Number(a.state === "running") || a.name.localeCompare(b.name));
  const up = rows.filter((c) => c.state === "running").length;
  return (
    <Panel title="Containers" action={<span className="font-mono text-[11px] text-text-3">{`${up} of ${rows.length} up`}</span>}>
      <table aria-label="Containers" className="w-full table-fixed border-t border-line text-[12px]">
        <thead>
          <tr className="text-left text-[11px] text-text-3">
            <th className="w-8 py-1.5 pl-3 font-normal" aria-label="State" />
            <th className="py-1.5 font-normal">Name</th>
            <th className="py-1.5 font-normal">Image</th>
            <th className="w-[150px] py-1.5 pr-3 font-normal">Status</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((c) => (
            <tr key={c.name} className="h-8 border-t border-line">
              <td className="pl-3">
                <Dot on={c.state === "running"} label={c.state} />
              </td>
              <td className="truncate text-text">{c.name}</td>
              <td className="truncate pr-3 font-mono text-[11px] text-text-3">{c.image}</td>
              <td className="truncate pr-3 text-text-2">{c.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <p className="border-t border-line px-3 py-3 text-[12px] text-text-3">No containers, or Docker isn't readable.</p>}
    </Panel>
  );
}
