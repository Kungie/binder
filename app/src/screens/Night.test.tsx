import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { demoSeed, memoryApi } from "../lib/memory";
import { type Screen, StoreProvider } from "../lib/store";

function setup(initialScreen: Screen = "tasks") {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen} updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

/** A todo task assigned to me in the demo board. */
async function mineInTodo(api: ReturnType<typeof memoryApi>) {
  const all = await api.listTasks({ kind: "all" }, false);
  return all.find((t) => t.status === "todo" && t.assignee === "me" && !t.night)!;
}

const card = (key: string) => screen.getByRole("button", { name: new RegExp(`^${key} `) });

describe("Night tasks", () => {
  it("marking_a_night_task_shows_the_moon", async () => {
    const { api, user } = setup();
    const t = await mineInTodo(api);
    await user.click(await screen.findByRole("button", { name: new RegExp(`^${t.key} `) }));
    const sheet = screen.getByRole("dialog", { name: t.key });
    await user.click(within(sheet).getByRole("switch", { name: "Night task" }));
    await waitFor(() => expect(within(sheet).getByRole("switch", { name: "Night task" })).toHaveAttribute("aria-checked", "true"));
    expect(within(sheet).getByRole("combobox", { name: "Assignee" })).toHaveValue("agent");
    expect(within(card(t.key)).getByRole("img", { name: "Night task" })).toBeInTheDocument();
    expect(within(card(t.key)).getByText("Tonight")).toBeInTheDocument();
  });

  it("the_night_tab_lists_night_tasks", async () => {
    const { api, user } = setup();
    const t = await mineInTodo(api);
    await screen.findByRole("button", { name: new RegExp(`^${t.key} `) });
    await api.updateTask(t.id, { night: true });
    act(() => api.emitTasksChanged());
    await user.click(await screen.findByRole("button", { name: "Night" }));
    await waitFor(() => expect(card(t.key)).toBeInTheDocument());
    const shown = screen.getAllByRole("button", { name: /^[A-Z]+-\d+ / });
    expect(shown).toHaveLength(1);
  });

  it("tasks_reload_when_the_app_says_so", async () => {
    const { api } = setup();
    const t = await mineInTodo(api);
    await screen.findByRole("button", { name: new RegExp(`^${t.key} `) });
    await api.updateTask(t.id, { title: "Changed in the background" });
    act(() => api.emitTasksChanged());
    expect(await screen.findByRole("button", { name: new RegExp(`^${t.key} Changed in the background`) })).toBeInTheDocument();
  });
});

describe("Settings › Claude Code", () => {
  it("pick_up_switch_saves", async () => {
    const { api, user } = setup("settings");
    const toggle = await screen.findByRole("switch", { name: "Pick up agent tasks" });
    expect(toggle).toHaveAttribute("aria-checked", "true");
    await user.click(toggle);
    await waitFor(async () => expect((await api.getConfig()).pick_up).toBe(false));
    expect(toggle).toHaveAttribute("aria-checked", "false");
  });

  it("night_setup_and_remove", async () => {
    const { user } = setup("settings");
    expect(await screen.findByText("Not set up")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Set up" }));
    expect(await screen.findByText("Runs at 03:00 on the server")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(await screen.findByText("Not set up")).toBeInTheDocument();
  });
});
