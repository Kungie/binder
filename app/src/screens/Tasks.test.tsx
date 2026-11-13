import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { type Store, StoreProvider, useStore } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

let store: Store | null = null;
function Grab() {
  store = useStore();
  return null;
}

async function setup(initialScreen: "tasks" | "overview" = "tasks") {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Grab />
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Tasks header", () => {
  it("header has All, Mine and Agent only", async () => {
    await setup();
    const tabs = await screen.findByRole("group", { name: "Task views" });
    expect(within(tabs).getAllByRole("button").map((b) => b.textContent)).toEqual(["All", "Mine", "Agent"]);
    expect(within(tabs).queryByRole("button", { name: "orbit" })).toBeNull();
  });

  it("a project view switches to All when Tasks opens", async () => {
    await setup("overview");
    act(() => store!.setView({ kind: "project", id: 1 }));
    act(() => store!.setScreen("tasks"));
    await screen.findByRole("group", { name: "Task views" });
    expect(store!.view).toEqual({ kind: "all" });
  });
});
