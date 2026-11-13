import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup(options = {}, initialScreen: "projects" | "settings" = "projects") {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  if (initialScreen === "projects") await screen.findByRole("list", { name: "Projects" });
  return { api, user };
}

const pick = async (user: ReturnType<typeof userEvent.setup>, name: string) =>
  user.click(within(screen.getByRole("list", { name: "Projects" })).getByRole("button", { name: new RegExp(`^${name}`) }));

describe("Projects", () => {
  it("creating_project_lists_it_with_prefix", async () => {
    const { user } = await setup();
    await user.click(screen.getByRole("button", { name: "New project" }));
    const dialog = screen.getByRole("dialog", { name: "New project" });
    await user.type(within(dialog).getByLabelText("Name"), "Şirket ağı");
    expect(within(dialog).getByLabelText("Prefix")).toHaveAttribute("placeholder", "SIR");
    await user.keyboard("{Enter}");
    const list = screen.getByRole("list", { name: "Projects" });
    await waitFor(() => expect(within(list).getByRole("button", { name: /^Şirket ağı/ })).toHaveTextContent("SIR"));
    expect(screen.queryByRole("dialog", { name: "New project" })).toBeNull();
  });

  it("start_project_turns_into_open_note", async () => {
    const { user } = await setup();
    await pick(user, "homelab");
    await user.click(await screen.findByRole("button", { name: "Start project" }));
    expect(await screen.findByRole("button", { name: "Open note" })).toBeInTheDocument();
  });

  it("adding_done_line_shows_in_log", async () => {
    const { user } = await setup();
    await pick(user, "orbit");
    const log = await screen.findByRole("list", { name: "Done log" });
    await waitFor(() => expect(within(log).getAllByRole("listitem")).toHaveLength(1));
    await user.type(screen.getByLabelText("Add a done line"), "Moved sync to a worker{Enter}");
    await waitFor(() => expect(within(log).getAllByRole("listitem")[0]).toHaveTextContent("Moved sync to a worker"));
    expect(screen.getByLabelText("Add a done line")).toHaveValue("");
  });

  it("disconnected_vault_shows_notice", async () => {
    const { user } = await setup({ vaultConnected: false });
    await pick(user, "homelab");
    expect(await screen.findByText(/Vault not connected/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Start project" }));
    expect(screen.getByRole("button", { name: "Start project" })).toBeInTheDocument();
  });
});

describe("Settings", () => {
  it("saves_the_vault_path", async () => {
    const { api, user } = await setup({}, "settings");
    const field = await screen.findByLabelText("Vault folder");
    await waitFor(() => expect(field).toHaveValue("/Volumes/vault"));
    await user.clear(field);
    await user.type(field, "/Volumes/notes");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(async () => expect((await api.getConfig()).vault).toBe("/Volumes/notes"));
  });
});
