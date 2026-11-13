import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup(initialScreen: "tasks" | "projects" = "tasks") {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  // These tests drive the list; the board has its own.
  localStorage.setItem("binder.tasks.layout.all", "list");
  localStorage.setItem("binder.tasks.layout.agent", "list");
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  if (initialScreen === "tasks") {
    const grid = await screen.findByRole("grid", { name: "Tasks" });
    await waitFor(() => expect(within(grid).getAllByRole("row").length).toBeGreaterThan(0));
    grid.focus();
  } else {
    await screen.findByRole("list", { name: "Projects" });
  }
  return { api, user };
}

const find = async (api: ReturnType<typeof memoryApi>, key: string) => (await api.listTasks({ kind: "all" }, true)).find((t) => t.key === key)!;

describe("review fixes", () => {
  it("escape_closes_new_task", async () => {
    const { user } = await setup();
    await user.keyboard("c");
    await screen.findByRole("dialog", { name: "New task" });
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "New task" })).toBeNull();
  });

  it("escape_closes_new_project", async () => {
    const { user } = await setup("projects");
    await user.click(screen.getByRole("button", { name: "New project" }));
    await screen.findByRole("dialog", { name: "New project" });
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "New project" })).toBeNull();
  });

  it("escape_saves_sheet_text", async () => {
    const { api, user } = await setup();
    await user.keyboard("{Enter}");
    const sheet = await screen.findByRole("dialog", { name: "ORB-4" });
    await user.click(within(sheet).getByLabelText("Notes"));
    await user.keyboard("hello");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "ORB-4" })).toBeNull();
    await waitFor(async () => expect((await find(api, "ORB-4")).notes).toMatch(/hover\.hello$/));
  });

  it("selection_is_dropped_when_rows_leave", async () => {
    const { api, user } = await setup();
    await user.keyboard("jx");
    await user.click(screen.getByRole("button", { name: "Agent" }));
    const grid = screen.getByRole("grid", { name: "Tasks" });
    grid.focus();
    await user.keyboard("s5");
    await waitFor(async () => expect((await find(api, "LAB-2")).status).toBe("canceled"));
    expect((await find(api, "TSK-2")).status).toBe("todo");
  });

  it("sheet_done_offers_done_line", async () => {
    const { user } = await setup();
    await user.keyboard("{Enter}");
    const sheet = await screen.findByRole("dialog", { name: "ORB-4" });
    await user.selectOptions(within(sheet).getByLabelText("Status"), "done");
    expect(await screen.findByLabelText("Log to orbit")).toHaveValue("Wire the sync status into the toolbar");
  });

  it("sheet_priority_field_shows_the_chip_and_opens_the_menu", async () => {
    const { api, user } = await setup();
    await user.keyboard("{Enter}");
    const sheet = await screen.findByRole("dialog", { name: "ORB-4" });
    const field = within(sheet).getByRole("button", { name: "Priority: High" });
    expect(field.querySelector("[data-chip]")?.textContent).toBe("High");
    await user.click(field);
    await user.click(within(screen.getByRole("listbox", { name: "Set priority" })).getByRole("option", { name: /Urgent/ }));
    await waitFor(async () => expect((await find(api, "ORB-4")).priority).toBe("urgent"));
  });

  it("new_task_takes_a_priority", async () => {
    const { api, user } = await setup();
    await user.keyboard("c");
    const dialog = screen.getByRole("dialog", { name: "New task" });
    await user.type(within(dialog).getByLabelText("Task title"), "Fix the hinge");
    await user.selectOptions(within(dialog).getByLabelText("Priority"), "urgent");
    await user.click(within(dialog).getByRole("button", { name: "Create task" }));
    await waitFor(async () => expect((await api.listTasks({ kind: "all" }, true)).find((t) => t.title === "Fix the hinge")?.priority).toBe("urgent"));
  });

  it("p_in_the_list_sets_priority", async () => {
    const { api, user } = await setup();
    await user.keyboard("p");
    await user.click(within(screen.getByRole("listbox", { name: "Set priority" })).getByRole("option", { name: /Low/ }));
    await waitFor(async () => expect((await find(api, "ORB-4")).priority).toBe("low"));
  });
});
