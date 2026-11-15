import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup() {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="tasks" updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  await screen.findByRole("region", { name: "In progress" });
  return { api, user };
}

const dialog = () => screen.getByRole("dialog", { name: "New task" });
const find = async (api: ReturnType<typeof memoryApi>, title: string) => (await api.listTasks({ kind: "all" }, true)).find((t) => t.title === title);

describe("New task", () => {
  it("new_task_sets_every_field", async () => {
    const { api, user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(dialog());
    await user.type(d.getByLabelText("Task title"), "Ship the stand");
    await user.type(d.getByLabelText("Notes"), "Ship it");
    await user.selectOptions(d.getByLabelText("Status"), "in_progress");
    await user.click(d.getByRole("button", { name: /^Priority:/ }));
    await user.click(within(screen.getByRole("listbox", { name: "Set priority" })).getByRole("option", { name: /High/ }));
    await user.selectOptions(d.getByLabelText("Project"), "2");
    await user.type(d.getByLabelText("Due date"), "2026-11-20");
    await user.type(d.getByLabelText("Due time"), "09:30");
    await user.click(d.getByRole("switch", { name: "Night task" }));
    await user.click(d.getByRole("button", { name: "Create task" }));
    await waitFor(async () => expect(await find(api, "Ship the stand")).toBeTruthy());
    const t = (await find(api, "Ship the stand"))!;
    expect(t).toMatchObject({ notes: "Ship it", status: "in_progress", priority: "high", project_id: 2, assignee: "agent", due: "2026-11-20", due_time: "09:30", night: true });
    expect(screen.queryByRole("dialog", { name: "New task" })).toBeNull();
  });

  it("night_assigns_the_agent_and_me_turns_night_off", async () => {
    const { user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(dialog());
    await user.click(d.getByRole("switch", { name: "Night task" }));
    expect(d.getByLabelText("Assignee")).toHaveValue("agent");
    await user.selectOptions(d.getByLabelText("Assignee"), "me");
    expect(d.getByRole("switch", { name: "Night task" })).toHaveAttribute("aria-checked", "false");
  });

  it("enter_in_notes_adds_a_line_and_cmd_enter_creates", async () => {
    const { api, user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(dialog());
    await user.type(d.getByLabelText("Task title"), "Two lines");
    await user.type(d.getByLabelText("Notes"), "one{Enter}two");
    expect(screen.getByRole("dialog", { name: "New task" })).toBeInTheDocument();
    expect(d.getByLabelText("Notes")).toHaveValue("one\ntwo");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    await waitFor(async () => expect((await find(api, "Two lines"))?.notes).toBe("one\ntwo"));
  });

  it("the_buttons_sit_in_a_footer", async () => {
    const { api, user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(dialog());
    const create = d.getByRole("button", { name: "Create task" });
    const cancel = d.getByRole("button", { name: "Cancel" });
    const footer = create.closest(".border-t");
    expect(footer).not.toBeNull();
    expect(footer).toContainElement(cancel);
    await user.type(d.getByLabelText("Task title"), "Never mind");
    await user.click(cancel);
    expect(screen.queryByRole("dialog", { name: "New task" })).toBeNull();
    expect(await find(api, "Never mind")).toBeUndefined();
  });

  it("the_controls_share_a_row_instead_of_stretching", async () => {
    const { user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(dialog());
    for (const label of ["Status", "Assignee", "Project", "Due date", "Due time"]) expect(d.getByLabelText(label)).not.toHaveClass("w-full");
  });

  it("a_board_column_still_presets_the_status", async () => {
    const { user } = await setup();
    await user.click(within(screen.getByRole("region", { name: "In progress" })).getByRole("button", { name: "New In progress task" }));
    expect(within(dialog()).getByLabelText("Status")).toHaveValue("in_progress");
  });
});
