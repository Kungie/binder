import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";
import { nextDue } from "../lib/repeat";

async function setup(prepare?: (api: ReturnType<typeof memoryApi>) => Promise<unknown>) {
  const api = memoryApi(demoSeed());
  await prepare?.(api);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="tasks" updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  await screen.findByRole("region", { name: "In progress" });
  return { api, user };
}

const all = (api: ReturnType<typeof memoryApi>) => api.listTasks({ kind: "all" }, true);

describe("Repeating tasks", () => {
  it("new_task_can_repeat", async () => {
    const { api, user } = await setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    const d = within(screen.getByRole("dialog", { name: "New task" }));
    await user.type(d.getByLabelText("Task title"), "Pay rent");
    await user.type(d.getByLabelText("Due date"), "2026-12-01");
    await user.selectOptions(d.getByLabelText("Repeat"), "monthly");
    await user.click(d.getByRole("button", { name: "Create task" }));
    await waitFor(async () => expect((await all(api)).find((t) => t.title === "Pay rent")?.repeat).toBe("monthly"));
  });

  it("finishing_one_brings_the_next", async () => {
    const { api } = await setup();
    const t = await api.createTask({ title: "Water plants", due: "2030-01-07", repeat: "weekly" });
    await api.updateTask(t.id, { status: "done" });
    const next = (await all(api)).filter((x) => x.title === "Water plants" && x.status === "todo");
    expect(next).toHaveLength(1);
    expect(next[0]).toMatchObject({ due: "2030-01-14", repeat: "weekly" });
    expect((await all(api)).find((x) => x.id === t.id)?.repeat).toBeNull();
  });

  it("the_card_says_it_repeats_and_the_sheet_changes_the_rule", async () => {
    let key = "";
    const { api, user } = await setup(async (api) => {
      key = (await api.createTask({ title: "Backup photos", repeat: "weekly", due: "2030-01-07" })).key;
    });
    const card = await screen.findByRole("button", { name: new RegExp(`^${key} `) });
    expect(within(card).getByText("Weekly")).toBeInTheDocument();
    await user.click(card);
    const sheet = await screen.findByRole("dialog", { name: key });
    const select = within(sheet).getByLabelText("Repeat") as HTMLSelectElement;
    expect(select.value).toBe("weekly");
    await user.selectOptions(select, "monthly");
    await waitFor(async () => expect((await all(api)).find((x) => x.key === key)?.repeat).toBe("monthly"));
    await user.selectOptions(select, "");
    await waitFor(async () => expect((await all(api)).find((x) => x.key === key)?.repeat).toBeNull());
  });

  it("next_due_matches_the_rules", () => {
    expect(nextDue("weekdays", "2026-11-20", "2026-11-20")).toBe("2026-11-23");
    expect(nextDue("monthly", "2027-01-31", "2027-01-31")).toBe("2027-02-28");
    expect(nextDue("weekly", "2026-10-26", "2026-11-16")).toBe("2026-11-23");
    expect(nextDue("yearly", null, "2026-11-16")).toBe("2027-11-16");
  });
});
