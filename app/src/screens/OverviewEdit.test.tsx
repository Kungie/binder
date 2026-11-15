import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup(api = memoryApi(demoSeed())) {
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="overview" updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

const column = (side: "left" | "right") => document.querySelector(`[data-column="${side}"]`) as HTMLElement;
const ids = (side: "left" | "right") => [...column(side).querySelectorAll("[data-widget]")].map((e) => e.getAttribute("data-widget"));

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
  document.elementFromPoint = at;
}

describe("Overview you arrange", () => {
  it("edit_shows_handles_and_done_hides_them", async () => {
    const { user } = setup();
    await waitFor(() => expect(ids("right")).toEqual(["server", "cloud", "printer", "open_tasks"]));
    expect(screen.queryByRole("button", { name: "Move Server" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByRole("button", { name: "Move Server" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("button", { name: "Move Server" })).toBeNull();
  });

  it("hide_removes_a_widget_and_add_brings_it_back", async () => {
    const { api, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Hide Cloud" }));
    expect(ids("right")).toEqual(["server", "printer", "open_tasks"]);
    await waitFor(async () => expect((await api.getConfig()).overview.right).toEqual(["server", "printer", "open_tasks"]));
    await user.click(screen.getByRole("button", { name: "Add widget" }));
    await user.click(within(screen.getByRole("menu", { name: "Add widget" })).getByRole("menuitem", { name: "Cloud" }));
    expect(ids("right")).toContain("cloud");
  });

  it("dragging_moves_a_widget_to_the_other_column", async () => {
    const { api, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    const target = column("left").querySelector('[data-widget="done_today"]') as HTMLElement;
    drag(screen.getByRole("button", { name: "Move Server" }), target);
    expect(ids("left")).toEqual(["numbers", "server", "done_today"]);
    expect(ids("right")).toEqual(["cloud", "printer", "open_tasks"]);
    await waitFor(async () => expect((await api.getConfig()).overview.left).toEqual(["numbers", "server", "done_today"]));
  });

  it("dropping_on_a_columns_empty_space_appends", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    drag(screen.getByRole("button", { name: "Move Numbers" }), column("right"));
    expect(ids("right")).toEqual(["server", "cloud", "printer", "open_tasks", "numbers"]);
  });

  it("reset_brings_back_the_default", async () => {
    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("button", { name: "Hide Server" }));
    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(ids("right")).toEqual(["server", "cloud", "printer", "open_tasks"]);
  });

  it("the_layout_is_saved_and_read_back", async () => {
    const api = memoryApi(demoSeed());
    await api.setConfig({ ...(await api.getConfig()), overview: { left: ["printer", "bogus", "printer"], right: ["done_today"] } });
    setup(api);
    await waitFor(() => expect(ids("left")).toEqual(["printer"]));
    expect(ids("right")).toEqual(["done_today"]);
  });

  it("sites_widget_counts_sites_up", async () => {
    const api = memoryApi(demoSeed());
    await api.setConfig({ ...(await api.getConfig()), overview: { left: ["sites"], right: [] } });
    const { user } = setup(api);
    const w = await screen.findByRole("region", { name: "Sites" });
    expect(await within(w).findByText("2 of 3 up")).toBeInTheDocument();
    await user.click(within(w).getByRole("button", { name: "Open" }));
    expect(await screen.findByRole("heading", { name: "Sites" })).toBeInTheDocument();
  });

  it("github_widget_counts_what_needs_you", async () => {
    const api = memoryApi(demoSeed());
    await api.setConfig({ ...(await api.getConfig()), overview: { left: ["github"], right: [] } });
    setup(api);
    const w = await screen.findByRole("region", { name: "GitHub" });
    expect(await within(w).findByText("2 pull requests, 1 issue need you")).toBeInTheDocument();
  });
});
