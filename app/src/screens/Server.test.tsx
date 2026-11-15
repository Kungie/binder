import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";
import type { ServerStatus } from "../lib/types";

function setup(options: MemoryOptions = {}, change?: (s: ServerStatus) => void) {
  const seed = demoSeed();
  if (change) change(seed.server!);
  const api = memoryApi(seed, options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="server">
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Server", () => {
  it("server_shows_containers_and_metrics", async () => {
    setup();
    const table = await screen.findByRole("table", { name: "Containers" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(10));
    expect(within(table).getByText("jellyfin")).toBeInTheDocument();
    for (const m of ["Load", "Memory", "Disk", "Temperature"]) expect(screen.getByRole("group", { name: new RegExp(`^${m}:`) })).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Running services" })).toHaveTextContent("tailscaled");
  });

  it("shows the server's power at the right end", async () => {
    setup();
    const power = await screen.findByRole("group", { name: "Power: 4.5 W" });
    const row = power.parentElement!;
    expect(row.lastElementChild).toBe(power);
    expect(within(power).getByText("Pi board")).toBeInTheDocument();
  });

  it("power reads None when the server has none", async () => {
    setup({}, (s) => (s.power_w = null));
    expect(await screen.findByRole("group", { name: "Power: None" })).toBeInTheDocument();
  });

  it("server_error_shows_last_read", async () => {
    const { api, user } = setup();
    await screen.findByRole("table", { name: "Containers" });
    api.setServerReachable(false);
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(await screen.findByText(/Can't reach lab/)).toHaveTextContent(/Last read \d\d:\d\d/);
    expect(screen.getByRole("table", { name: "Containers" })).toBeInTheDocument();
  });

  it("server_without_target_asks_for_one", async () => {
    const { api, user } = setup({ server: null });
    const field = await screen.findByLabelText("SSH target");
    await user.type(field, "lab");
    await user.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(async () => expect((await api.getConfig()).server).toBe("lab"));
    expect(await screen.findByRole("table", { name: "Containers" })).toBeInTheDocument();
  });

  it("load shows as a percentage", async () => {
    setup({}, (s) => ((s.load = [0.74, 0.8, 0.62]), (s.cores = 2)));
    const load = await screen.findByRole("group", { name: /^Load:/ });
    expect(load).toHaveTextContent("37%");
    expect(load).toHaveTextContent(/5 min 40%\s+15 min 31%/);
  });

  it("load falls back to raw numbers without cores", async () => {
    setup({}, (s) => ((s.load = [0.74, 0.8, 0.62]), (s.cores = 0)));
    const load = await screen.findByRole("group", { name: /^Load:/ });
    expect(load).toHaveTextContent("0.74");
    expect(load).toHaveTextContent(/0\.80\s+0\.62/);
  });

  const many = (n: number) => (s: ServerStatus) => (s.services = Array.from({ length: n }, (_, i) => ({ name: `svc${i}.service`, description: `Service ${i}` })));

  it("shows 13 services and Show all", async () => {
    setup({}, many(20));
    const list = await screen.findByRole("list", { name: "Running services" });
    await waitFor(() => expect(within(list).getAllByRole("listitem").filter((l) => l.textContent?.startsWith("svc"))).toHaveLength(13));
    expect(within(list).getByRole("button", { name: "Show all (20)" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Services" })).toHaveTextContent("20");
  });

  it("Show all lists every service and Show fewer folds it", async () => {
    const { user } = setup({}, many(20));
    const list = await screen.findByRole("list", { name: "Running services" });
    await user.click(await within(list).findByRole("button", { name: "Show all (20)" }));
    expect(within(list).getAllByRole("listitem").filter((l) => l.textContent?.startsWith("svc"))).toHaveLength(20);
    await user.click(within(list).getByRole("button", { name: "Show fewer" }));
    expect(within(list).getAllByRole("listitem").filter((l) => l.textContent?.startsWith("svc"))).toHaveLength(13);
  });

  it("no button with 13 or fewer", async () => {
    setup({}, many(13));
    const list = await screen.findByRole("list", { name: "Running services" });
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(13));
    expect(within(list).queryByRole("button")).toBeNull();
  });

  it("overview says load in percent", async () => {
    const seed = demoSeed();
    seed.server!.load = [0.74, 0.8, 0.62];
    seed.server!.cores = 2;
    render(
      <StoreProvider api={memoryApi(seed)} initialScreen="overview">
        <Shell />
      </StoreProvider>,
    );
    expect(await screen.findByText("load 37%")).toBeInTheDocument();
  });
});

describe("What uses the server", () => {
  it("load_lists_processes_by_cpu", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Load" }));
    const list = await screen.findByRole("region", { name: "Processes by CPU" });
    expect(within(list).getAllByRole("row")[1]).toHaveTextContent("ollama");
    expect(screen.getByRole("button", { name: "Load" })).toHaveAttribute("aria-pressed", "true");
  });

  it("memory_lists_processes_by_memory", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Memory" }));
    const list = await screen.findByRole("region", { name: "Processes by memory" });
    expect(within(list).getAllByRole("row")[1]).toHaveTextContent("1024");
  });

  it("pressing_again_closes", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Load" }));
    await screen.findByRole("region", { name: "Processes by CPU" });
    await user.click(screen.getByRole("button", { name: "Load" }));
    expect(screen.queryByRole("region", { name: "Processes by CPU" })).toBeNull();
  });

  it("disk_lists_folders", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Disk" }));
    const list = await screen.findByRole("region", { name: "Largest folders" });
    expect(within(list).getAllByRole("row")[1]).toHaveTextContent("/srv/cloud");
  });

  it("disk_box_adds_the_sd_card", async () => {
    const seed = demoSeed().server!;
    const used = seed.disk_used + seed.sd_used!;
    const total = seed.disk_size + seed.sd_size!;
    setup();
    const disk = await screen.findByRole("group", { name: `Disk: ${Math.round((used / total) * 100)}%` });
    expect(disk).toHaveTextContent("SSD and SD card");
  });

  it("disk_box_without_sd_is_as_before", async () => {
    setup({}, (s) => ((s.sd_size = null), (s.sd_used = null)));
    const disk = await screen.findByRole("group", { name: /^Disk:/ });
    expect(disk).not.toHaveTextContent("SD card");
  });
});

describe("Containers", () => {
  it("pressing_a_container_shows_its_log", async () => {
    const { user } = setup();
    const table = await screen.findByRole("table", { name: "Containers" });
    await user.click(await within(table).findByRole("button", { name: "jellyfin" }));
    const log = await screen.findByRole("region", { name: "jellyfin log" });
    expect(within(log).getByText(/jellyfin started/)).toBeInTheDocument();
    expect(within(log).getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("restart_asks_first_and_cancel_does_nothing", async () => {
    const { api, user } = setup();
    const table = await screen.findByRole("table", { name: "Containers" });
    await user.click(await within(table).findByRole("button", { name: "jellyfin" }));
    const log = await screen.findByRole("region", { name: "jellyfin log" });
    await user.click(within(log).getByRole("button", { name: "Restart" }));
    expect(within(log).getByText("Restart jellyfin? Its connections drop for a moment.")).toBeInTheDocument();
    await user.click(within(log).getByRole("button", { name: "Cancel" }));
    expect(api.restarted).toEqual([]);
  });

  it("restart_shows_restarting_then_the_status", async () => {
    const { api, user } = setup();
    const table = await screen.findByRole("table", { name: "Containers" });
    await user.click(await within(table).findByRole("button", { name: "jellyfin" }));
    const log = await screen.findByRole("region", { name: "jellyfin log" });
    await user.click(within(log).getByRole("button", { name: "Restart" }));
    await user.click(within(log).getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(api.restarted).toEqual(["jellyfin"]));
    await waitFor(() => expect(within(log).queryByText("Restarting…")).toBeNull());
    expect(within(log).getByRole("button", { name: "Restart" })).toBeEnabled();
  });
});
