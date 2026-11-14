import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { dueLabel, plainText } from "../lib/tasks";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup(seed = demoSeed()) {
  const api = memoryApi(seed);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="tasks">
      <Shell />
    </StoreProvider>,
  );
  await screen.findByRole("region", { name: "In progress" });
  await waitFor(() => expect(cards("Todo").length).toBeGreaterThan(0));
  return { api, user };
}

const column = (name: string) => screen.getByRole("region", { name });
const cards = (name: string) => within(column(name)).queryAllByRole("button", { name: /^[A-Z]+-\d+ / });
const card = (key: string) => screen.getByRole("button", { name: new RegExp(`^${key} `) });
const byKey = async (api: ReturnType<typeof memoryApi>, key: string) => (await api.listTasks({ kind: "all" }, true)).find((t) => t.key === key)!;

/**
 * A pointer drag, as the app sees one: the webview's file-drop handler takes every HTML5 drag, so the board moves
 * cards with pointer events. jsdom has no layout, so `elementFromPoint` is pointed at the target for the drag.
 */
function drag(from: HTMLElement, to: HTMLElement) {
  const at = document.elementFromPoint;
  document.elementFromPoint = () => to;
  const fire = (target: EventTarget, type: string, x: number) =>
    act(() => {
      target.dispatchEvent(new MouseEvent(type, { bubbles: true, button: 0, clientX: x, clientY: x }));
    });
  fire(from, "pointerdown", 10);
  fire(document, "pointermove", 20);
  fire(document, "pointermove", 40);
  fire(document, "pointerup", 40);
  fire(to, "click", 40);
  document.elementFromPoint = at;
}

describe("Board", () => {
  it("board_renders_columns_with_counts", async () => {
    await setup();
    for (const [name, n] of [
      ["Backlog", 1],
      ["Todo", 4],
      ["In progress", 1],
      ["Done", 1],
    ] as const) {
      expect(within(column(name)).getByLabelText(n === 1 ? "1 task" : `${n} tasks`)).toBeInTheDocument();
      expect(cards(name)).toHaveLength(n);
    }
  });

  it("done_shows_the_last_week_then_all", async () => {
    const { user } = await setup();
    expect(cards("Done").map((c) => c.getAttribute("data-key"))).toEqual(["LAB-1"]);
    await user.click(screen.getByRole("button", { name: "Show all" }));
    expect(cards("Done")).toHaveLength(2);
  });

  it("card_shows_two_line_notes_and_chips", async () => {
    await setup();
    const c = card("ORB-4");
    expect(within(c).getByText("Show the sync state beside the title, with the last error on hover.")).toHaveClass("line-clamp-2");
    const chips = [...c.querySelectorAll("[data-chip]")].map((x) => x.textContent);
    expect(chips).toEqual(["In progress", "High", "orbit"]);
    expect([...card("LAB-2").querySelectorAll("[data-chip]")].map((x) => x.textContent)).toEqual(["Todo", "Medium", "Tomorrow 9 AM", "homelab", "Claude"]);
  });

  it("drop_on_done_sets_status_and_prompts_once", async () => {
    const { api } = await setup();
    drag(card("ORB-3"), column("Done"));
    await waitFor(async () => expect((await byKey(api, "ORB-3")).status).toBe("done"));
    expect(await screen.findAllByLabelText("Log to orbit")).toHaveLength(1);
    expect(within(column("Done")).getByRole("button", { name: /^ORB-3 / })).toBeInTheDocument();
  });

  it("cards_are_not_html5_draggable", async () => {
    await setup();
    // The webview's file-drop handler swallows HTML5 drags in the app, so they must never start.
    expect(card("ORB-3")).not.toHaveAttribute("draggable", "true");
  });

  it("a_drag_doesnt_open_the_card", async () => {
    await setup();
    drag(card("ORB-3"), column("Done"));
    await waitFor(() => expect(within(column("Done")).getByRole("button", { name: /^ORB-3 / })).toBeInTheDocument());
    expect(screen.queryByRole("dialog", { name: "ORB-3" })).toBeNull();
  });

  it("reorder_in_column_keeps_status_and_no_prompt", async () => {
    const { api } = await setup();
    const before = cards("Todo").map((c) => c.getAttribute("data-key"));
    const last = before[before.length - 1]!;
    drag(card(last), card(before[0]!));
    await waitFor(() => expect(cards("Todo").map((c) => c.getAttribute("data-key"))).toEqual([last, ...before.slice(0, -1)]));
    expect((await byKey(api, last)).status).toBe("todo");
    expect(screen.queryByLabelText(/^Log to/)).toBeNull();
  });

  it("attention_dot_shows_and_clears_on_open", async () => {
    const { user } = await setup();
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByLabelText("Needs attention")).toBeInTheDocument();
    expect(within(card("LAB-1")).getByLabelText("Needs attention")).toBeInTheDocument();
    await user.click(card("LAB-1"));
    expect(await screen.findByRole("dialog", { name: "LAB-1" })).toBeInTheDocument();
    await waitFor(() => expect(within(nav).queryByLabelText("Needs attention")).toBeNull());
  });

  it("switch_remembers_list_view", async () => {
    const { user } = await setup();
    await user.click(screen.getByRole("button", { name: "List" }));
    expect(await screen.findByRole("grid", { name: "Tasks" })).toBeInTheDocument();
    expect(localStorage.getItem("binder.tasks.layout.all")).toBe("list");
    await user.click(screen.getByRole("button", { name: "Mine" }));
    expect(await screen.findByRole("region", { name: "Todo" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "All" }));
    expect(await screen.findByRole("grid", { name: "Tasks" })).toBeInTheDocument();
  });

  it("search_filters_cards", async () => {
    const { user } = await setup();
    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "toolbar");
    await waitFor(() => expect(cards("Todo")).toHaveLength(0));
    expect(cards("In progress")).toHaveLength(1);
    await user.clear(screen.getByRole("searchbox", { name: "Search tasks" }));
    await user.type(screen.getByRole("searchbox", { name: "Search tasks" }), "error on hover");
    await waitFor(() => expect(cards("In progress")).toHaveLength(1));
  });

  it("plus_in_a_column_creates_with_its_status", async () => {
    const { api, user } = await setup();
    await user.click(within(column("Backlog")).getByRole("button", { name: "New Backlog task" }));
    await user.type(within(screen.getByRole("dialog", { name: "New task" })).getByLabelText("Task title"), "Sketch a stand{Enter}");
    await waitFor(async () => expect((await api.listTasks({ kind: "all" }, true)).find((t) => t.title === "Sketch a stand")?.status).toBe("backlog"));
  });

  it("p_opens_the_priority_menu_on_a_focused_card_and_sets_it", async () => {
    const { api, user } = await setup();
    const first = cards("Backlog")[0]!;
    first.focus();
    await user.keyboard("p");
    const menu = screen.getByRole("listbox", { name: "Set priority" });
    await user.click(within(menu).getByRole("option", { name: /High/ }));
    await waitFor(async () => expect((await byKey(api, "ORB-2")).priority).toBe("high"));
    expect(screen.queryByRole("listbox", { name: "Set priority" })).toBeNull();
  });

  it("keys_move_across_cards_and_columns", async () => {
    const { user } = await setup();
    const first = cards("Todo")[0]!;
    first.focus();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(cards("Todo")[1]);
    await user.keyboard("{ArrowLeft}");
    expect(document.activeElement).toBe(cards("Backlog")[0]);
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(document.activeElement).toBe(cards("In progress")[0]);
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "ORB-4" })).toBeInTheDocument();
  });
});

describe("dueLabel", () => {
  const today = "2026-11-13"; // a Friday
  it.each([
    [["2026-11-13", null], "Today", false],
    [["2026-11-13", "14:00"], "Today 2 PM", false],
    [["2026-11-13", "09:30"], "Today 9:30 AM", false],
    [["2026-11-13", "00:00"], "Today 12 AM", false],
    [["2026-11-14", null], "Tomorrow", false],
    [["2026-11-17", null], "Tue", false],
    [["2026-11-19", "18:00"], "Thu 6 PM", false],
    [["2026-11-20", null], "20 Nov", false],
    [["2026-11-12", null], "12 Nov", true],
    [["2025-12-01", null], "1 Dec 2025", true],
  ] as const)("%j → %s", ([due, time], text, overdue) => {
    expect(dueLabel(due, time, today)).toEqual({ text, overdue });
  });
});

describe("plainText", () => {
  it("strips markdown to readable text", () => {
    expect(plainText("# Plan\n- **Fix** the [toolbar](https://x.y) `spacing`\n> quoted\n\n```sh\nmake\n```")).toBe("Plan Fix the toolbar spacing quoted make");
  });
});
