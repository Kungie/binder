import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup(options = {}, initialScreen: "projects" | "settings" = "projects", seed = demoSeed()) {
  const api = memoryApi(seed, options);
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

describe("Repo", () => {
  it("project_shows_repo_row", async () => {
    const { user } = await setup();
    await pick(user, "orbit");
    const row = await screen.findByRole("group", { name: "Git" });
    for (const t of ["main", "3 changed", "↑1 ↓0", "Fix the toolbar spacing"]) expect(within(row).getByText(t)).toBeInTheDocument();
  });
});

describe("Claude Code", () => {
  it("connect_toggles_in_settings", async () => {
    const { api, user } = await setup({}, "settings");
    const panel = await screen.findByRole("region", { name: "Claude Code" });
    await user.click(await within(panel).findByRole("button", { name: "Connect" }));
    expect(await within(panel).findByRole("button", { name: "Disconnect" })).toBeInTheDocument();
    expect(await api.claudeHooks()).toBe(true);
    await user.click(within(panel).getByRole("button", { name: "Disconnect" }));
    expect(await within(panel).findByRole("button", { name: "Connect" })).toBeInTheDocument();
    expect(await api.claudeHooks()).toBe(false);
  });

  it("sync_button_adds_github_projects", async () => {
    const { user } = await setup();
    const list = screen.getByRole("list", { name: "Projects" });
    expect(within(list).queryByRole("button", { name: /^weather-cli/ })).toBeNull();
    expect(screen.getByText("Not synced yet")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Sync" }));
    expect(await within(list).findByRole("button", { name: /^weather-cli/ })).toBeInTheDocument();
    expect(screen.getByText(/^Synced/)).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: /^dotfiles/ }).querySelector("[aria-label='Private']")).not.toBeNull();
  });

  it("sign_in_line_when_gh_missing", async () => {
    const { user } = await setup({}, "projects", { ...demoSeed(), github: null });
    await user.click(screen.getByRole("button", { name: "Sync" }));
    expect(await screen.findByText("Sign in with gh auth login to bring in your GitHub repos.")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Projects" })).toBeInTheDocument();
  });

  it("archived_projects_are_grouped", async () => {
    const { user } = await setup();
    const list = screen.getByRole("list", { name: "Projects" });
    expect(within(list).queryByRole("button", { name: /^old-site/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Archived 1" }));
    expect(within(list).getByRole("button", { name: /^old-site/ })).toBeInTheDocument();
  });

  it("colour_picker_sets_project_colour", async () => {
    const { user } = await setup();
    await pick(user, "orbit");
    const picker = screen.getByRole("radiogroup", { name: "Colour" });
    expect(within(picker).getAllByRole("radio")).toHaveLength(8);
    await user.click(within(picker).getByRole("radio", { name: "Cyan" }));
    await waitFor(() => expect(within(picker).getByRole("radio", { name: "Cyan" })).toHaveAttribute("aria-checked", "true"));
    const row = within(screen.getByRole("list", { name: "Projects" })).getByRole("button", { name: /^orbit/ });
    expect(row.querySelector("[data-color]")?.getAttribute("data-color")).toBe("#26B5CE");
  });

  it("github_header_links_the_repo", async () => {
    const { user } = await setup();
    await pick(user, "orbit");
    expect(screen.getByRole("link", { name: "demo-dev/orbit" })).toHaveAttribute("href", "https://github.com/demo-dev/orbit");
    expect(screen.getByText("TypeScript")).toBeInTheDocument();
    expect(screen.getByText("pushed 1 day ago")).toBeInTheDocument();
    expect(screen.getByText("Local folder")).toBeInTheDocument();
  });
});
