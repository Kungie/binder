import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { demoSeed, memoryApi } from "../lib/memory";
import { type Screen, StoreProvider } from "../lib/store";

function setup(initialScreen: Screen = "overview") {
  const user = userEvent.setup();
  render(
    <StoreProvider api={memoryApi(demoSeed())} initialScreen={initialScreen} updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { user };
}

/** jsdom has no matchMedia; these tests stand in for a Mac with motion on (or off). */
function motion(on: boolean) {
  vi.stubGlobal("matchMedia", (q: string) => ({ matches: q.includes("reduce") ? !on : false, media: q, addEventListener() {}, removeEventListener() {} }));
}

afterEach(() => vi.unstubAllGlobals());

describe("Motion", () => {
  it("each_screen_comes_in_with_its_own_animation", async () => {
    const { user } = setup();
    const first = await screen.findByTestId("screen");
    expect(first).toHaveClass("screen-in");
    expect(first).toHaveAttribute("data-screen", "overview");
    await user.click(within(screen.getByRole("navigation", { name: "Main" })).getByRole("button", { name: /^Tasks/ }));
    const next = await screen.findByTestId("screen");
    expect(next).toHaveAttribute("data-screen", "tasks");
    expect(next).not.toBe(first);
    expect(next).toHaveClass("screen-in");
  });

  it("the_task_sheet_slides_in_and_out", async () => {
    motion(true);
    const { user } = setup("tasks");
    await user.click(await screen.findByText("Write release notes for 0.4"));
    const sheet = await screen.findByRole("dialog", { name: "ORB-3" });
    expect(sheet).toHaveClass("sheet-in");
    await user.keyboard("{Escape}");
    expect(sheet).toHaveClass("sheet-out");
    expect(screen.getByRole("dialog", { name: "ORB-3" })).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "ORB-3" })).not.toBeInTheDocument());
  });

  it("with_reduce_motion_the_sheet_closes_at_once", async () => {
    motion(false);
    const { user } = setup("tasks");
    await user.click(await screen.findByText("Write release notes for 0.4"));
    await screen.findByRole("dialog", { name: "ORB-3" });
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "ORB-3" })).not.toBeInTheDocument();
  });

  it("dialogs_and_menus_pop_in", async () => {
    const { user } = setup();
    await user.click(screen.getByRole("button", { name: "New task" }));
    expect(await screen.findByRole("dialog", { name: "New task" })).toHaveClass("pop-in");
  });
});
