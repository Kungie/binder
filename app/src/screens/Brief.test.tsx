import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { type Screen, StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup(initialScreen: Screen = "overview", prepare?: (api: ReturnType<typeof memoryApi>) => void) {
  const api = memoryApi(demoSeed());
  prepare?.(api);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen} updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Morning brief", () => {
  it("the_brief_shows_each_kind_of_line", async () => {
    setup();
    const brief = await screen.findByRole("region", { name: "Morning brief" });
    expect(within(brief).getByText(/^The server restarted at \d\d:\d\d$/)).toBeInTheDocument();
    expect(within(brief).getByText(/^web stopped at \d\d:\d\d, running again at \d\d:\d\d$/)).toBeInTheDocument();
    expect(within(brief).getByText("LAB-2 done")).toBeInTheDocument();
    expect(within(brief).getByRole("img", { name: "Night task" })).toBeInTheDocument();
    expect(within(brief).getByText("case-lid is done")).toBeInTheDocument();
    expect(within(brief).getByText(/^[A-Z]+-\d+ from GitHub: /)).toBeInTheDocument();
  });

  it("dismiss_hides_the_brief", async () => {
    const { user } = setup();
    const brief = await screen.findByRole("region", { name: "Morning brief" });
    await user.click(within(brief).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Morning brief" })).toBeNull());
  });

  it("no_items_no_card", async () => {
    setup("overview", (api) => vi.spyOn(api, "brief").mockResolvedValue({ since: 0, shown: true, items: [] }));
    await screen.findByRole("heading", { name: "Overview" });
    await new Promise((r) => setTimeout(r, 20));
    expect(screen.queryByRole("region", { name: "Morning brief" })).toBeNull();
  });

  it("a_brief_task_line_opens_its_sheet", async () => {
    const { user } = setup();
    const brief = await screen.findByRole("region", { name: "Morning brief" });
    await user.click(within(brief).getByRole("button", { name: /LAB-2 done/ }));
    expect(await screen.findByRole("dialog", { name: "LAB-2" })).toBeInTheDocument();
  });

  it("a_container_line_opens_the_server", async () => {
    const { user } = setup();
    const brief = await screen.findByRole("region", { name: "Morning brief" });
    await user.click(within(brief).getByRole("button", { name: /web stopped/ }));
    expect(await screen.findByRole("heading", { name: "Server" })).toBeInTheDocument();
  });
});

describe("GitHub on tasks and projects", () => {
  it("issue_tasks_show_their_number", async () => {
    const { api, user } = setup("tasks");
    const card = await screen.findByRole("button", { name: /^ORB-2 / });
    expect(within(card).getByText("#12")).toBeInTheDocument();
    await user.click(card);
    const sheet = await screen.findByRole("dialog", { name: "ORB-2" });
    await user.click(within(sheet).getByRole("button", { name: "Open issue" }));
    expect(api.openedLinks).toEqual(["https://github.com/demo-dev/orbit/issues/12"]);
  });

  it("projects_show_ci_and_open_prs", async () => {
    setup("projects");
    const list = await screen.findByRole("list", { name: "Projects" });
    const orbit = within(list).getByRole("button", { name: /orbit/ });
    expect(within(orbit).getByRole("img", { name: "CI failing" })).toBeInTheDocument();
    expect(within(orbit).getByText("2 open PRs")).toBeInTheDocument();
  });

  it("print_notifications_switch_saves", async () => {
    const { api, user } = setup("settings");
    const sw = await screen.findByRole("switch", { name: "Notify me about prints" });
    expect(sw).toHaveAttribute("aria-checked", "true");
    await user.click(sw);
    await waitFor(async () => expect((await api.getConfig()).print_notify).toBe(false));
  });

  it("the_brief_hides_at_noon_without_a_reload", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { api } = setup("overview");
      const real = api.brief.bind(api);
      let noon = false;
      vi.spyOn(api, "brief").mockImplementation(async () => ({ ...(await real()), shown: !noon }));
      await screen.findByRole("region", { name: "Morning brief" });
      noon = true;
      await vi.advanceTimersByTimeAsync(5 * 60_000);
      await waitFor(() => expect(screen.queryByRole("region", { name: "Morning brief" })).toBeNull());
    } finally {
      vi.useRealTimers();
    }
  });

  it("site_and_disk_lines_read_plainly", async () => {
    const { briefText } = await import("../components/Brief");
    const at = new Date(2026, 10, 17, 3, 10).getTime() / 1000;
    expect(briefText({ kind: "site", name: "blog", at, back_at: at + 600 })).toBe("blog was down from 03:10, up again at 03:20");
    expect(briefText({ kind: "site", name: "blog", at, back_at: null })).toBe("blog went down at 03:10");
    expect(briefText({ kind: "disk", name: "Server disk", percent: 92, at })).toBe("Server disk is 92% full");
  });
});
