import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";

function setup(options: MemoryOptions = {}) {
  const api = memoryApi(demoSeed(), options);
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
});
