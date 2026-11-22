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

  it("lists friends and adds one with a password", async () => {
    const { api, user } = setup();
    const panel = await screen.findByRole("region", { name: "Friends" });
    expect(await within(panel).findByText("ada")).toBeInTheDocument();
    expect(within(panel).getByText("linus")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Add friend" }));
    await user.type(within(panel).getByRole("textbox", { name: "User name" }), "grace");
    await user.type(within(panel).getByLabelText("Password"), "short");
    await user.click(within(panel).getByRole("button", { name: "Add" }));
    expect(await within(panel).findByText("A password is at least 8 characters.")).toBeInTheDocument();
    await user.type(within(panel).getByLabelText("Password"), " but longer");
    await user.click(within(panel).getByRole("button", { name: "Add" }));
    expect(await within(panel).findByText("grace")).toBeInTheDocument();
    expect((await api.friendsList()).map((f) => f.name)).toEqual(["ada", "grace", "linus"]);
  });

  it("removes a friend only after asking", async () => {
    const { api, user } = setup();
    const panel = await screen.findByRole("region", { name: "Friends" });
    await user.click(await within(panel).findByRole("button", { name: "Remove linus" }));
    expect(within(panel).getByText(/Remove linus and their devices/)).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(within(panel).queryByText("linus")).not.toBeInTheDocument());
    expect((await api.friendsList()).map((f) => f.name)).toEqual(["ada"]);
  });

  it("resets a friend's password", async () => {
    const { user } = setup();
    const panel = await screen.findByRole("region", { name: "Friends" });
    await user.click(await within(panel).findByRole("button", { name: "Reset password of ada" }));
    await user.type(within(panel).getByLabelText("New password"), "fresh password");
    await user.click(within(panel).getByRole("button", { name: "Save" }));
    expect(await within(panel).findByText("Password changed for ada.")).toBeInTheDocument();
  });

  it("asks for the OAuth client before friends can sign in", async () => {
    const { user } = setup({ friendsService: { installed: true, configured: false, users: 0 }, friends: [] });
    const panel = await screen.findByRole("region", { name: "Friends" });
    expect(await within(panel).findByText(/needs a Tailscale OAuth client/)).toBeInTheDocument();
    await user.type(within(panel).getByRole("textbox", { name: "Client id" }), "kid");
    await user.type(within(panel).getByLabelText("Client secret"), "tskey-client-x");
    await user.click(within(panel).getByRole("button", { name: "Save client" }));
    await waitFor(() => expect(within(panel).queryByText(/needs a Tailscale OAuth client/)).not.toBeInTheDocument());
  });

  it("says when the service isn't on the server", async () => {
    setup({ friendsService: { installed: false, configured: false, users: 0 } });
    const panel = await screen.findByRole("region", { name: "Friends" });
    expect(await within(panel).findByText(/sign-in service isn't running/)).toBeInTheDocument();
  });

  it("changes the OAuth client, or keeps it on Cancel", async () => {
    const { user } = setup();
    const panel = await screen.findByRole("region", { name: "Friends" });
    await user.click(await within(panel).findByRole("button", { name: "Change client" }));
    expect(within(panel).getByRole("textbox", { name: "Client id" })).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Cancel" }));
    expect(within(panel).queryByRole("textbox", { name: "Client id" })).not.toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Change client" }));
    await user.type(within(panel).getByRole("textbox", { name: "Client id" }), "kid2");
    await user.type(within(panel).getByLabelText("Client secret"), "tskey-client-y");
    await user.click(within(panel).getByRole("button", { name: "Save client" }));
    expect(await within(panel).findByText("Client saved.")).toBeInTheDocument();
    expect(within(panel).queryByRole("textbox", { name: "Client id" })).not.toBeInTheDocument();
  });
});
