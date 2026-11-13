import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

// Rows in the demo's task table: ORB-4, TSK-2, LAB-2 (the agent's), ...
async function open(keys: string, name: string) {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="tasks">
      <Shell />
    </StoreProvider>,
  );
  const grid = await screen.findByRole("grid", { name: "Tasks" });
  await waitFor(() => expect(within(grid).getAllByRole("row").length).toBeGreaterThan(3));
  grid.focus();
  await user.keyboard(keys);
  const sheet = await screen.findByRole("dialog", { name });
  return { api, user, sheet };
}

describe("AgentRun", () => {
  it("agent_task_shows_run_controls", async () => {
    const { sheet } = await open("jj{Enter}", "LAB-2");
    const run = within(sheet).getByRole("region", { name: "Agent run" });
    expect(within(run).getByLabelText("Folder")).toHaveValue("~/homelab");
    expect(within(run).getByRole("radio", { name: "Edits only" })).toBeChecked();
    expect(within(run).getByRole("button", { name: "Run" })).toBeInTheDocument();
  });

  it("my_task_hides_them", async () => {
    const { sheet } = await open("j{Enter}", "TSK-2");
    expect(within(sheet).queryByRole("region", { name: "Agent run" })).toBeNull();
  });

  it("full_access_needs_the_warning_seen", async () => {
    const { api, user, sheet } = await open("jj{Enter}", "LAB-2");
    const run = within(sheet).getByRole("region", { name: "Agent run" });
    expect(within(run).queryByText("Runs without asking. Use only for tasks you trust.")).toBeNull();
    await user.click(within(run).getByRole("radio", { name: "Full access" }));
    expect(within(run).getByText("Runs without asking. Use only for tasks you trust.")).toBeInTheDocument();
    await user.click(within(run).getByRole("button", { name: "Run" }));
    await waitFor(async () => expect((await api.runsFor(12))[0]?.access).toBe("full"));
  });

  it("run_then_poll_shows_tail_and_done", async () => {
    const { api, user, sheet } = await open("jj{Enter}", "LAB-2");
    const run = within(sheet).getByRole("region", { name: "Agent run" });
    await user.click(within(run).getByRole("button", { name: "Run" }));
    expect(await within(run).findByText("Finished")).toBeInTheDocument();
    expect(within(run).getByLabelText("Log")).toHaveTextContent("Pruned 4 images.");
    await waitFor(async () => expect((await api.listTasks({ kind: "all" }, true)).find((t) => t.key === "LAB-2")?.status).toBe("done"));
  });
});
