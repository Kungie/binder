import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

// Open tasks in the demo, in the core's order.
const ORDER = ["ORB-4", "TSK-2", "LAB-2", "TSK-1", "ORB-3", "ORB-2"];

async function setup(options = {}) {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="tasks">
      <Shell />
    </StoreProvider>,
  );
  const grid = await screen.findByRole("grid", { name: "Tasks" });
  await waitFor(() => expect(within(grid).getAllByRole("row")).toHaveLength(ORDER.length));
  grid.focus();
  return { api, user, grid };
}

const focusedKey = (grid: HTMLElement) => within(grid).getAllByRole("row").find((r) => r.getAttribute("aria-selected") === "true")?.dataset.key;
const statusOf = async (api: ReturnType<typeof memoryApi>, key: string) => (await api.listTasks({ kind: "all" }, true)).find((t) => t.key === key)!;

describe("TaskTable keys", () => {
  it("j_and_k_move_focus_and_clamp", async () => {
    const { user, grid } = await setup();
    expect(focusedKey(grid)).toBe("ORB-4");
    await user.keyboard("jj");
    expect(focusedKey(grid)).toBe("LAB-2");
    await user.keyboard("{ArrowDown}jjjjjj");
    expect(focusedKey(grid)).toBe("ORB-2");
    await user.keyboard("kkkkkkkkk");
    expect(focusedKey(grid)).toBe("ORB-4");
  });

  it("enter_opens_focused_task", async () => {
    const { user } = await setup();
    await user.keyboard("j{Enter}");
    const sheet = await screen.findByRole("dialog", { name: "TSK-2" });
    expect(within(sheet).getByLabelText("Title")).toHaveValue("Order PETG, black and white");
  });

  it("c_calls_create", async () => {
    const { user } = await setup();
    await user.keyboard("c");
    expect(await screen.findByRole("dialog", { name: "New task" })).toBeInTheDocument();
  });

  it("s_then_digit_sets_status", async () => {
    const { api, user } = await setup();
    await user.keyboard("jjs");
    expect(screen.getByRole("listbox", { name: "Set status" })).toBeInTheDocument();
    await user.keyboard("3");
    await waitFor(async () => expect((await statusOf(api, "LAB-2")).status).toBe("in_progress"));
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("x_selects_and_s_applies_to_selection", async () => {
    const { api, user } = await setup();
    await user.keyboard("jxjxs1");
    await waitFor(async () => {
      expect((await statusOf(api, "TSK-2")).status).toBe("backlog");
      expect((await statusOf(api, "LAB-2")).status).toBe("backlog");
    });
    expect((await statusOf(api, "ORB-4")).status).toBe("in_progress");
  });

  it("a_toggles_assignee", async () => {
    const { api, user } = await setup();
    await user.keyboard("a");
    await waitFor(async () => expect((await statusOf(api, "ORB-4")).assignee).toBe("agent"));
  });
});

describe("Done prompt", () => {
  it("done_with_project_shows_prompt_and_logs", async () => {
    const { api, user } = await setup();
    await user.keyboard("s4");
    const input = await screen.findByLabelText("Log to orbit");
    expect(input).toHaveValue("Wire the sync status into the toolbar");
    expect(input).toHaveFocus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(api.done[1].map((l) => l.text)).toContain("Wire the sync status into the toolbar"));
    expect(screen.queryByLabelText("Log to orbit")).toBeNull();
  });

  it("done_prompt_escape_skips", async () => {
    const { api, user } = await setup();
    await user.keyboard("s4");
    await screen.findByLabelText("Log to orbit");
    await user.keyboard("{Escape}");
    expect(screen.queryByLabelText("Log to orbit")).toBeNull();
    expect(api.done[1]).toHaveLength(1);
  });

  it("done_without_project_asks_nothing", async () => {
    const { user } = await setup();
    await user.keyboard("js4");
    await waitFor(() => expect(screen.queryByRole("listbox")).toBeNull());
    expect(screen.queryByText(/Log to/)).toBeNull();
  });

  it("vault_missing_shows_message", async () => {
    const { user } = await setup({ vaultConnected: false });
    await user.keyboard("s4");
    const input = await screen.findByLabelText("Log to orbit");
    await user.keyboard("{Enter}");
    expect(await screen.findByText(/Vault not connected/)).toBeInTheDocument();
    expect(input).toHaveValue("Wire the sync status into the toolbar");
  });
});
