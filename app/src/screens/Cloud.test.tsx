import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import type { Screen } from "../lib/store";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup(options = {}, initialScreen: Screen = "cloud") {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

const files = () => screen.getByRole("table", { name: "Files" });
const names = () => within(files()).queryAllByRole("row").slice(1).map((r) => r.getAttribute("data-name"));
const row = (name: string) => within(files()).getByRole("row", { name: new RegExp(`^${name.replace(".", "\\.")}\\b`) });

async function ready() {
  await waitFor(() => expect(names().length).toBeGreaterThan(0));
}

describe("Cloud", () => {
  it("not_mounted_offers_connect", async () => {
    const { user } = await setup({ cloudMounted: false });
    expect(await screen.findByRole("heading", { name: "Cloud not connected" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Set the share address in Settings.")).toBeInTheDocument();
  });

  it("lists_folders_first_and_enters_folder", async () => {
    const { user } = await setup();
    await ready();
    expect(names()).toEqual(["Backups", "Documents", "Photos", "Videos", "README.md"]);
    await user.dblClick(row("Photos"));
    await waitFor(() => expect(names()).toEqual(["2026"]));
    const crumbs = screen.getByRole("navigation", { name: "Path" });
    expect(within(crumbs).getByRole("button", { name: "Photos" })).toHaveAttribute("aria-current", "page");
    await user.click(within(crumbs).getByRole("button", { name: "Cloud" }));
    await waitFor(() => expect(names()).toContain("README.md"));
  });

  it("columns_sort_with_folders_first", async () => {
    const { user } = await setup();
    await ready();
    await user.click(screen.getByRole("button", { name: "Name" }));
    expect(names()).toEqual(["Videos", "Photos", "Documents", "Backups", "README.md"]);
  });

  it("upload_shows_progress_then_lists_file", async () => {
    const { api } = await setup();
    await ready();
    api.dropFiles(["/Users/x/Desktop/clip.mov"]);
    const tray = await screen.findByRole("region", { name: "Transfers" });
    expect(within(tray).getByText("clip.mov")).toBeInTheDocument();
    expect(within(tray).getByRole("progressbar", { name: "clip.mov" })).toBeInTheDocument();
    await waitFor(() => expect(names()).toContain("clip.mov"));
  });

  it("a_name_clash_asks_once_per_batch", async () => {
    const { api, user } = await setup();
    await ready();
    api.dropFiles(["/tmp/README.md", "/tmp/notes.txt"]);
    const ask = await screen.findByRole("alertdialog", { name: "1 item already exists here" });
    await user.click(within(ask).getByRole("button", { name: "Keep both" }));
    await waitFor(() => expect(names()).toEqual(["Backups", "Documents", "Photos", "Videos", "notes.txt", "README 2.md", "README.md"]));
  });

  it("delete_moves_to_trash_and_undo_restores", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard("{Meta>}{Backspace}{/Meta}");
    await waitFor(() => expect(names()).not.toContain("README.md"));
    expect(screen.getByText("Moved to trash.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(names()).toContain("README.md"));
  });

  it("trash_view_restores_and_empties_with_confirm", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("Documents"));
    await user.keyboard("{Meta>}{Backspace}{/Meta}");
    await waitFor(() => expect(names()).not.toContain("Documents"));
    await user.click(screen.getByRole("button", { name: "Trash" }));
    const trash = await screen.findByRole("table", { name: "Trash" });
    await user.click(within(trash).getByRole("button", { name: "Restore Documents" }));
    expect(await screen.findByText("The trash is empty.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Files" }));
    await waitFor(() => expect(names()).toContain("Documents"));
    await user.click(row("Backups"));
    await user.keyboard("{Meta>}{Backspace}{/Meta}");
    await waitFor(() => expect(names()).not.toContain("Backups"));
    await user.click(screen.getByRole("button", { name: "Trash" }));
    await user.click(await screen.findByRole("button", { name: "Empty trash" }));
    expect(screen.getByText("Empty the trash? This can't be undone.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Empty" }));
    expect(await screen.findByText("The trash is empty.")).toBeInTheDocument();
  });

  it("new_folder_and_rename", async () => {
    const { user } = await setup();
    await ready();
    await user.click(screen.getByRole("button", { name: "New folder" }));
    await user.type(screen.getByRole("textbox", { name: "Folder name" }), "Taxes{Enter}");
    await waitFor(() => expect(names()).toContain("Taxes"));
    await user.click(row("Taxes"));
    await user.keyboard("{Enter}");
    const input = screen.getByRole("textbox", { name: "Rename Taxes" });
    await user.clear(input);
    await user.type(input, "Receipts{Enter}");
    await waitFor(() => expect(names()).toContain("Receipts"));
    expect(names()).not.toContain("Taxes");
  });

  it("dragging_a_row_onto_a_folder_moves_it", async () => {
    const { fireEvent } = await import("@testing-library/react");
    await setup();
    await ready();
    const data: Record<string, string> = {};
    const dt = { setData: (k: string, v: string) => (data[k] = v), getData: (k: string) => data[k] ?? "", types: [] };
    fireEvent.dragStart(row("README.md"), { dataTransfer: dt });
    fireEvent.dragOver(row("Documents"), { dataTransfer: dt });
    fireEvent.drop(row("Documents"), { dataTransfer: dt });
    await waitFor(() => expect(names()).not.toContain("README.md"));
  });

  it("overview_shows_cloud_space", async () => {
    await setup({}, "overview");
    const panel = await screen.findByRole("region", { name: "Cloud" });
    expect(await within(panel).findByText(/free of 2 TB/)).toBeInTheDocument();
    await waitFor(() => expect(within(panel).getAllByRole("listitem").map((l) => l.textContent?.split(/\d/)[0].trim())).toEqual(["Videos", "Photos", "Documents"]));
  });

  it("share_address_from_settings_lets_connect_open_it", async () => {
    const { api, user } = await setup({ cloudMounted: false }, "settings");
    const share = await screen.findByLabelText("Cloud share address");
    await user.type(share, "ftp://x");
    await user.click(screen.getByRole("button", { name: "Save Cloud settings" }));
    expect(await screen.findByText("The share address starts with smb://.")).toBeInTheDocument();
    await user.clear(share);
    await user.type(share, "smb://lab/cloud");
    await user.click(screen.getByRole("button", { name: "Save Cloud settings" }));
    await waitFor(async () => expect((await api.getConfig()).cloud_share).toBe("smb://lab/cloud"));
    await user.click(within(screen.getByRole("navigation", { name: "Main" })).getByRole("button", { name: "Cloud" }));
    await user.click(await screen.findByRole("button", { name: "Connect" }));
    await waitFor(() => expect(api.cloudConnects).toBe(1));
  });
});
