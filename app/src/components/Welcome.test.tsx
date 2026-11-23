import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";

function setup(options: MemoryOptions = {}, initialScreen: "overview" | "settings" | "server" = "overview") {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("First-run setup", () => {
  it("walks through the server, the vault and gh, then stays closed", async () => {
    const { api, user } = setup({ onboarded: false, server: null, setupChecks: { gh: true, gh_signed_in: false, tailscale: true } });
    const dialog = await screen.findByRole("dialog", { name: "Set up Binder" });
    expect(within(dialog).getByText("Welcome to Binder")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Start" }));
    await user.type(within(dialog).getByRole("textbox", { name: "SSH target" }), "me@10.0.7.99");
    await user.click(within(dialog).getByRole("button", { name: "Connect" }));
    expect(await within(dialog).findByText(/Couldn't connect/)).toBeInTheDocument();
    await user.clear(within(dialog).getByRole("textbox", { name: "SSH target" }));
    await user.type(within(dialog).getByRole("textbox", { name: "SSH target" }), "me@10.0.7.2");
    await user.click(within(dialog).getByRole("button", { name: "Connect" }));
    const vault = await within(dialog).findByRole("textbox", { name: "Vault folder" });
    await user.clear(vault);
    await user.type(vault, "/Users/me/Vault");
    await user.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(await within(dialog).findByText("gh is installed, not signed in")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Next" }));
    await user.click(within(dialog).getByRole("button", { name: "Start using Binder" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Set up Binder" })).not.toBeInTheDocument());
    const c = await api.getConfig();
    expect(c.server).toBe("me@10.0.7.2");
    expect(c.vault).toBe("/Users/me/Vault");
    expect(c.onboarded).toBe(true);
  });

  it("doesn't show once a server is set or setup was done", async () => {
    setup({ onboarded: false });
    await screen.findByRole("heading", { name: "Overview" }).catch(() => null);
    expect(screen.queryByRole("dialog", { name: "Set up Binder" })).not.toBeInTheDocument();
  });

  it("can be skipped and run again from Settings", async () => {
    const { api, user } = setup({ onboarded: false, server: null }, "settings");
    const dialog = await screen.findByRole("dialog", { name: "Set up Binder" });
    await user.click(within(dialog).getByRole("button", { name: "Skip setup" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect((await api.getConfig()).onboarded).toBe(true);
    await user.click(screen.getByRole("button", { name: "Run setup again" }));
    expect(await screen.findByRole("dialog", { name: "Set up Binder" })).toBeInTheDocument();
  });
});

describe("Screen guides", () => {
  it("covers the window the first time a screen opens, and Got it remembers", async () => {
    const { api, user } = setup({ tips: true }, "server");
    const guide = await screen.findByRole("dialog", { name: "About Servers" });
    expect(within(guide).getByRole("img", { name: "The Servers screen" })).toBeInTheDocument();
    expect(within(guide).getByText("A home server you reach with SSH")).toBeInTheDocument();
    expect(await within(guide).findByText("Ready")).toBeInTheDocument();
    await user.click(within(guide).getByRole("button", { name: "Got it" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "About Servers" })).not.toBeInTheDocument());
    expect((await api.getConfig()).tips_hidden).toContain("server");
  });

  it("offers Set it up when something is missing, then shows the screen's steps", async () => {
    const { user } = setup({ tips: true, server: null }, "server");
    const guide = await screen.findByRole("dialog", { name: "About Servers" });
    expect(await within(guide).findByText("Not yet")).toBeInTheDocument();
    await user.click(within(guide).getByRole("button", { name: "Set it up" }));
    expect(await screen.findByRole("region", { name: "Connect the home server" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy ssh user@host true" })).toBeInTheDocument();
  });

  it("opens again with ?", async () => {
    const { user } = setup({}, "server");
    await screen.findByRole("heading", { name: "Servers" });
    expect(screen.queryByRole("dialog", { name: "About Servers" })).not.toBeInTheDocument();
    await user.keyboard("?");
    const guide = await screen.findByRole("dialog", { name: "About Servers" });
    await user.keyboard("{Escape}");
    await waitFor(() => expect(guide).not.toBeInTheDocument());
  });

  it("Show guides again brings every guide back", async () => {
    const { api, user } = setup({}, "settings");
    await user.click(await screen.findByRole("button", { name: "Show guides again" }));
    await waitFor(async () => expect((await api.getConfig()).tips_hidden).toEqual([]));
  });
});
