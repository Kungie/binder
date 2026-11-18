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
    <StoreProvider api={api} initialScreen="vpn">
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("VPN", () => {
  it("lists each exit node as a region, the one in use first in line", async () => {
    setup();
    const regions = await screen.findByRole("radiogroup", { name: "Regions" });
    const vps = await within(regions).findByRole("radio", { name: /vps Frankfurt/ });
    expect(vps).toBeChecked();
    expect(within(regions).getByRole("radio", { name: /vps-sg Singapore/ })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "VPN" })).toBeChecked();
    expect(screen.getByRole("img", { name: /Traffic goes from Amsterdam to Frankfurt/ })).toBeInTheDocument();
  });

  it("turns off and on again through the same exit node", async () => {
    const { api, user } = setup();
    const toggle = await screen.findByRole("switch", { name: "VPN" });
    await waitFor(() => expect(toggle).toBeChecked());
    await user.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    await user.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(api.vpnCalls).toEqual([null, "100.64.0.4"]);
  });

  it("picking another region while on moves the traffic there", async () => {
    const { api, user } = setup();
    const regions = await screen.findByRole("radiogroup", { name: "Regions" });
    await user.click(await within(regions).findByRole("radio", { name: /vps-sg/ }));
    await waitFor(() => expect(within(regions).getByRole("radio", { name: /vps-sg/ })).toBeChecked());
    expect(api.vpnCalls).toEqual(["100.64.0.5"]);
  });

  it("a region's place can be set by hand", async () => {
    const { api, user } = setup();
    await user.selectOptions(await screen.findByRole("combobox", { name: "Place of vps" }), "nue");
    await waitFor(async () => expect((await api.getConfig()).vpn_regions).toEqual({ vps: "nue" }));
    expect(await screen.findByRole("radio", { name: /vps Nuremberg/ })).toBeInTheDocument();
  });

  it("shows the traffic through the exit node", async () => {
    setup();
    const stats = await screen.findByRole("region", { name: "Statistics" });
    expect(await within(stats).findByText(/^Downloaded$/)).toBeInTheDocument();
    expect(within(stats).getByText(/MB$/, { selector: "[data-stat=down]" })).toBeInTheDocument();
  });

  it("says how to add an exit node when there is none", async () => {
    setup({ tailnet: { running: true, peers: [] } });
    expect(await screen.findByText(/No exit nodes on your tailnet/)).toBeInTheDocument();
  });
});
