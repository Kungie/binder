import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

async function setup() {
  const user = userEvent.setup();
  render(
    <StoreProvider api={memoryApi(demoSeed())}>
      <Shell />
    </StoreProvider>,
  );
  await screen.findByRole("group", { name: /^Open: / });
  return user;
}

const options = () => within(screen.getByRole("dialog", { name: "Command palette" })).getAllByRole("option").map((o) => o.textContent);

describe("Command palette", () => {
  it("cmd_k_opens_and_filters", async () => {
    const user = await setup();
    await user.keyboard("{Meta>}k{/Meta}");
    expect(screen.getByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
    await user.keyboard("proj");
    await waitFor(() => expect(options()).toEqual(expect.arrayContaining(["Go to Projects", "New project"])));
    expect(options()).not.toContain("Go to Tasks");
  });

  it("enter_navigates", async () => {
    const user = await setup();
    await user.keyboard("{Meta>}k{/Meta}");
    await user.keyboard("go to tasks{Enter}");
    expect(await screen.findByRole("grid", { name: "Tasks" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
  });

  it("opens_a_project", async () => {
    const user = await setup();
    await user.keyboard("{Meta>}k{/Meta}");
    await user.keyboard("open homelab{Enter}");
    expect(await screen.findByRole("heading", { name: "homelab" })).toBeInTheDocument();
  });

  it("escape_closes", async () => {
    const user = await setup();
    await user.keyboard("{Meta>}k{/Meta}");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull();
  });
});

describe("Overview", () => {
  it("overview_counts_open_and_in_progress", async () => {
    await setup();
    expect(screen.getByRole("group", { name: "Open: 6" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "In progress: 1" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Done this week: 2" })).toBeInTheDocument();
  });

  it("shows_open_tasks_and_done_today", async () => {
    await setup();
    expect(await screen.findByRole("grid", { name: "Open tasks" })).toBeInTheDocument();
    expect(await screen.findByText("Keep sync off by default until conflicts are handled.")).toBeInTheDocument();
  });
});
