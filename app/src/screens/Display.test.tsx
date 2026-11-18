import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { demoSeed, memoryApi } from "../lib/memory";
import { type Screen, StoreProvider } from "../lib/store";

function setup(initialScreen: Screen = "settings") {
  const api = memoryApi(demoSeed());
  const zoom = vi.spyOn(api, "setZoom");
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen} updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user, zoom };
}

describe("Interface size", () => {
  it("picking_a_size_applies_it", async () => {
    const { user, zoom } = setup();
    await user.click(await screen.findByRole("button", { name: "125%" }));
    expect(zoom).toHaveBeenCalledWith(125);
    await waitFor(() => expect(screen.getByRole("button", { name: "125%" })).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByRole("button", { name: "100%" })).toHaveAttribute("aria-pressed", "false");
  });

  it("shortcuts_step_the_size", async () => {
    const { api, user, zoom } = setup("overview");
    await waitFor(async () => expect((await api.getConfig()).zoom).toBe(100));
    await user.keyboard("{Meta>}={/Meta}");
    await waitFor(() => expect(zoom).toHaveBeenLastCalledWith(110));
    await user.keyboard("{Meta>}0{/Meta}");
    await waitFor(() => expect(zoom).toHaveBeenLastCalledWith(100));
    await user.keyboard("{Meta>}-{/Meta}");
    expect((await api.getConfig()).zoom).toBe(100);
  });
});

describe("Container notifications", () => {
  it("the_watch_switch_saves", async () => {
    const { api, user } = setup();
    const toggle = await screen.findByRole("switch", { name: "Notify me about containers" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await user.click(toggle);
    await waitFor(async () => expect((await api.getConfig()).watch).toBe(false));
  });

  it("an_alert_opens_the_server_screen", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { api } = setup("overview");
    await screen.findByRole("button", { name: "Servers" });
    act(() => api.emitWatchAlert("web stopped"));
    expect(await screen.findByRole("heading", { name: "Servers" })).toBeInTheDocument();
    vi.restoreAllMocks();
  });

  it("an_alert_leaves_the_screen_while_the_window_is_in_use", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const { api } = setup("overview");
    await screen.findByRole("button", { name: "Servers" });
    act(() => api.emitWatchAlert("web stopped"));
    expect(screen.queryByRole("heading", { name: "Servers" })).not.toBeInTheDocument();
    vi.restoreAllMocks();
  });
});
