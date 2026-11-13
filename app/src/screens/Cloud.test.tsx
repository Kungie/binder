import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
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

const menu = () => screen.getByRole("menu");
const menuLabels = () => within(menu()).getAllByRole("menuitem").map((m) => m.getAttribute("aria-label"));
const pickItem = async (user: ReturnType<typeof userEvent.setup>, label: string) => user.click(within(menu()).getByRole("menuitem", { name: label }));
const enter = async (user: ReturnType<typeof userEvent.setup>, name: string) => {
  await user.dblClick(row(name));
  await waitFor(() => expect(screen.getByRole("navigation", { name: "Path" })).toHaveTextContent(name));
};

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

  it("right-click on an item selects it and shows its menu", async () => {
    const { user } = await setup();
    await ready();
    await user.pointer({ keys: "[MouseRight]", target: row("README.md") });
    expect(row("README.md")).toHaveAttribute("aria-selected", "true");
    expect(menuLabels()).toEqual([
      "Open",
      "Open with",
      "Quick Look",
      "Rename",
      "Duplicate",
      "Copy",
      "Cut",
      "Move to…",
      "Save to…",
      'Compress "README.md"',
      "Copy path",
      "Get info",
      "Move to trash",
    ]);
    expect(within(menu()).getByRole("menuitem", { name: "Duplicate" })).toHaveTextContent("⌘D");
    await user.keyboard("{Escape}");
    await enter(user, "Backups");
    await user.pointer({ keys: "[MouseRight]", target: row("old-site.zip") });
    expect(menuLabels()).toContain("Extract here");
    await user.keyboard("{Escape}");
    await user.pointer({ keys: "[MouseRight]", target: row("home-assistant.tar") });
    expect(menuLabels()).not.toContain("Extract here");
  });

  it("a menu on several items hides what needs one", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("Backups"));
    await user.keyboard("{Shift>}");
    await user.click(row("Photos"));
    await user.keyboard("{/Shift}");
    await user.pointer({ keys: "[MouseRight]", target: row("Documents") });
    const labels = menuLabels();
    expect(labels).toContain("Compress 3 items");
    expect(labels).not.toContain("Rename");
    expect(labels).not.toContain("Get info");
    expect(labels).not.toContain("Open with");
  });

  it("right-click on empty space shows the folder menu", async () => {
    const { user } = await setup();
    await ready();
    const empty = screen.getByTestId("cloud-empty-space");
    await user.pointer({ keys: "[MouseRight]", target: empty });
    expect(menuLabels()).toEqual(["New folder", "Upload…", "Show hidden"]);
    await user.keyboard("{Escape}");
    await user.pointer({ keys: "[MouseRight]", target: row("README.md") });
    await pickItem(user, "Copy");
    await user.pointer({ keys: "[MouseRight]", target: empty });
    expect(menuLabels()).toEqual(["New folder", "Paste", "Upload…", "Show hidden"]);
  });

  it("copy then paste copies, cut then paste moves", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard("{Meta>}c{/Meta}");
    await enter(user, "Documents");
    await user.keyboard("{Meta>}v{/Meta}");
    await waitFor(() => expect(names()).toContain("README.md"));
    await user.click(within(screen.getByRole("navigation", { name: "Path" })).getByRole("button", { name: "Cloud" }));
    await waitFor(() => expect(names()).toContain("README.md"));
    await user.click(row("Videos"));
    await user.keyboard("{Meta>}x{/Meta}");
    await enter(user, "Backups");
    await user.keyboard("{Meta>}v{/Meta}");
    await waitFor(() => expect(names()).toContain("Videos"));
    await user.click(within(screen.getByRole("navigation", { name: "Path" })).getByRole("button", { name: "Cloud" }));
    await waitFor(() => expect(names()).not.toContain("Videos"));
  });

  it("pasting onto a taken name asks first", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard("{Meta>}c{/Meta}{Meta>}v{/Meta}");
    const ask = await screen.findByRole("alertdialog", { name: "1 item already exists here" });
    await user.click(within(ask).getByRole("button", { name: "Keep both" }));
    await waitFor(() => expect(names()).toContain("README 2.md"));
  });

  it("duplicate", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard("{Meta>}d{/Meta}");
    await waitFor(() => expect(names()).toContain("README copy.md"));
  });

  it("move to trash works on the selection", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("Videos"));
    await user.keyboard("{Meta>}");
    await user.click(row("README.md"));
    await user.keyboard("{/Meta}");
    await user.pointer({ keys: "[MouseRight]", target: row("README.md") });
    await pickItem(user, "Move to trash");
    await waitFor(() => expect(names()).toEqual(["Backups", "Documents", "Photos"]));
    await user.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(names()).toEqual(["Backups", "Documents", "Photos", "Videos", "README.md"]));
  });

  it("double-click on a zip extracts it", async () => {
    const { api, user } = await setup();
    await ready();
    await enter(user, "Backups");
    await user.dblClick(row("old-site.zip"));
    await waitFor(() => expect(names()).toContain("old-site"));
    expect(api.cloudOpened).not.toContain("Backups/old-site.zip");
  });

  it("get info shows size and item count", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("Photos"));
    await user.keyboard("{Meta>}i{/Meta}");
    const info = await screen.findByRole("dialog", { name: "Info" });
    expect(await within(info).findByText("9.4 MB, 3 items")).toBeInTheDocument();
    expect(within(info).getByText("/Volumes/cloud/Photos")).toBeInTheDocument();
    expect(within(info).getByText("Folder")).toBeInTheDocument();
  });

  it("copy path writes the Mac path", async () => {
    const { user } = await setup();
    // After setup, which puts its own clipboard on navigator.
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    await ready();
    await enter(user, "Photos");
    await enter(user, "2026");
    await user.pointer({ keys: "[MouseRight]", target: row("lake.jpg") });
    await pickItem(user, "Copy path");
    expect(writeText).toHaveBeenCalledWith("/Volumes/cloud/Photos/2026/lake.jpg");
  });

  it("space calls quick look", async () => {
    const { api, user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard(" ");
    expect(api.quickLooked).toEqual(["README.md"]);
  });

  it("keyboard shortcuts", async () => {
    const { user } = await setup();
    await ready();
    await user.click(row("README.md"));
    await user.keyboard("{Meta>}a{/Meta}");
    expect(within(files()).getAllByRole("row", { selected: true })).toHaveLength(5);
    await user.keyboard("{Escape}");
    expect(within(files()).queryAllByRole("row", { selected: true })).toHaveLength(0);
    await user.keyboard("{Shift>}{Meta>}n{/Meta}{/Shift}");
    expect(screen.getByRole("textbox", { name: "Folder name" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.click(row("README.md"));
    await user.keyboard("{Shift>}{ArrowUp}{/Shift}");
    expect(within(files()).getAllByRole("row", { selected: true })).toHaveLength(2);
    await user.keyboard("{Meta>}{Backspace}{/Meta}");
    await waitFor(() => expect(names()).toEqual(["Backups", "Documents", "Photos"]));
  });

  it("move to… picks a folder", async () => {
    const { user } = await setup();
    await ready();
    await user.pointer({ keys: "[MouseRight]", target: row("README.md") });
    await pickItem(user, "Move to…");
    const picker = await screen.findByRole("dialog", { name: "Move to" });
    await user.click(await within(picker).findByRole("button", { name: "Documents" }));
    await user.click(within(picker).getByRole("button", { name: "Move here" }));
    await waitFor(() => expect(names()).not.toContain("README.md"));
  });

  it("an unfinished upload shows in the trash without restore", async () => {
    const seed = demoSeed();
    seed.cloud!.trash = [{ id: "1-0", origin: ".binder-partial-9-0-big.iso", deleted: 1, unfinished: true, entry: { name: "big.iso", rel: "1-0/x", kind: "file", size: 5, modified: 1 } }];
    const api = memoryApi(seed);
    const user = userEvent.setup();
    render(
      <StoreProvider api={api} initialScreen="cloud">
        <Shell />
      </StoreProvider>,
    );
    await ready();
    await user.click(screen.getByRole("button", { name: "Trash" }));
    const trash = await screen.findByRole("table", { name: "Trash" });
    expect(within(trash).getByText("Unfinished upload: big.iso")).toBeInTheDocument();
    expect(within(trash).queryByRole("button", { name: /Restore/ })).toBeNull();
  });

  it("the space bar shows the disk of the folder on screen", async () => {
    const seed = demoSeed();
    seed.cloud!.entries.push({ name: "SD", rel: "SD", kind: "dir", size: 0, modified: 1 });
    const api = memoryApi(seed);
    const user = userEvent.setup();
    render(
      <StoreProvider api={api} initialScreen="cloud">
        <Shell />
      </StoreProvider>,
    );
    await ready();
    expect(await screen.findByText(/free of 2 TB/)).toBeInTheDocument();
    await enter(user, "SD");
    expect(await screen.findByText(/free of 29 GB/)).toBeInTheDocument();
  });

  it("the tray counts files for a zip", async () => {
    const { api, user } = await setup();
    await ready();
    await enter(user, "Photos");
    await user.pointer({ keys: "[MouseRight]", target: row("2026") });
    await pickItem(user, 'Compress "2026"');
    const tray = await screen.findByRole("region", { name: "Transfers" });
    await waitFor(() => expect(within(tray).getByText("Done, 3 files")).toBeInTheDocument());
    void api;
  });

  it("settings show the server folder and the fast path", async () => {
    const { api, user } = await setup({}, "settings");
    const field = await screen.findByLabelText("Cloud folder on the server");
    await waitFor(() => expect(field).toHaveValue("/srv/cloud"));
    expect(await screen.findByText("Fast path: SSH")).toBeInTheDocument();
    await user.clear(field);
    await user.type(field, "/srv/files");
    await user.click(screen.getByRole("button", { name: "Save Cloud settings" }));
    await waitFor(async () => expect((await api.getConfig()).cloud_remote).toBe("/srv/files"));
  });
});
