import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";
import { localDay } from "../lib/tasks";
import type { DoneLine } from "../lib/types";

function setup(done: Record<number, DoneLine[]>) {
  const seed = { ...demoSeed(), done };
  const api = memoryApi(seed);
  render(
    <StoreProvider api={api} initialScreen="overview">
      <Shell />
    </StoreProvider>,
  );
  return { seed, user: userEvent.setup() };
}

const day = localDay();
const at = (time: string, text: string, date = day): DoneLine => ({ date, time, text });

describe("Done today", () => {
  it("done today shows time, project dot and text, newest first", async () => {
    const { seed } = setup({ 1: [at("09:10", "Early one."), at("08:00", "Yesterday.", "2000-01-01")], 2: [at("15:30", "Late one.")] });
    const list = await screen.findByRole("list", { name: "Done today" });
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(2));
    const [first, second] = within(list).getAllByRole("listitem");
    const p2 = seed.projects.find((p) => p.id === 2)!;
    expect(first).toHaveTextContent("15:30");
    expect(first).toHaveTextContent(p2.name);
    expect(first).toHaveTextContent("Late one.");
    expect(first.querySelector("[data-dot]")).toHaveStyle({ backgroundColor: p2.color });
    expect(second).toHaveTextContent("09:10");
    expect(second).toHaveTextContent("Early one.");
  });

  it("shows 8 then Show all", async () => {
    const lines = Array.from({ length: 10 }, (_, i) => at(`1${i}:00`, `Line ${i}.`));
    const { user } = setup({ 1: lines });
    const list = await screen.findByRole("list", { name: "Done today" });
    await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(8));
    expect(within(list).getAllByRole("listitem")[0]).toHaveTextContent("Line 9.");
    await user.click(screen.getByRole("button", { name: "Show all (10)" }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(10);
  });

  it("empty says Nothing logged today.", async () => {
    setup({});
    expect(await screen.findByText("Nothing logged today.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Show all/ })).toBeNull();
  });
});

describe("Overview layout and mascot", () => {
  it("done today comes before open tasks", async () => {
    setup({ 1: [at("09:10", "Early one.")] });
    const done = await screen.findByRole("list", { name: "Done today" });
    const open = await screen.findByText("Open tasks");
    expect(done.compareDocumentPosition(open) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("an empty done today shows the mascot", async () => {
    setup({});
    const line = await screen.findByText("Nothing logged today.");
    expect(line.closest("div")?.parentElement?.querySelector("[data-mascot]")).not.toBeNull();
  });

  it("the sidebar shows the mascot mark", async () => {
    setup({});
    const nav = await screen.findByRole("navigation", { name: "Main" });
    expect(nav.querySelector("[data-mascot-mark]")).not.toBeNull();
  });
});
