import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup(withFolder = false) {
  const seed = demoSeed();
  if (withFolder) seed.projects[0].agent_dir = "~/code/orbit";
  const api = memoryApi(seed);
  const user = userEvent.setup({ advanceTimers: (ms) => void (vi.isFakeTimers() && vi.advanceTimersByTime(ms)) });
  render(
    <StoreProvider api={api} initialScreen="github" updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

const list = () => screen.findByRole("list", { name: "Pull requests" });
const detail = () => screen.findByRole("region", { name: /^#\d+/ });

afterEach(() => vi.useRealTimers());

describe("GitHub screen", () => {
  it("sits_under_projects_in_the_sidebar", () => {
    setup();
    const names = within(screen.getByRole("navigation", { name: "Main" })).getAllByRole("button").map((b) => b.textContent ?? "");
    const projects = names.findIndex((n) => n.startsWith("Projects"));
    expect(names[projects + 1]).toMatch(/^GitHub/);
  });

  it("needs_you_is_the_default_and_lists_both_kinds", async () => {
    const { user } = setup();
    expect(screen.getByRole("radio", { name: "Needs you" })).toBeChecked();
    const pulls = await list();
    expect(within(pulls).getAllByRole("listitem").map((li) => li.textContent)).toEqual([expect.stringContaining("#41"), expect.stringContaining("#40")]);
    await user.click(screen.getByRole("tab", { name: /^Issues/ }));
    const issues = await screen.findByRole("list", { name: "Issues" });
    expect(within(issues).getAllByRole("listitem")).toHaveLength(1);
    await user.click(screen.getByRole("radio", { name: "All open" }));
    await waitFor(() => expect(within(screen.getByRole("list", { name: "Issues" })).getAllByRole("listitem")).toHaveLength(2));
  });

  it("a_row_opens_its_detail", async () => {
    const { user } = setup();
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    expect(within(d).getByText("feature/sync-dot → main")).toBeInTheDocument();
    expect(within(d).getByText("+120 −34 in 6 files")).toBeInTheDocument();
    expect(within(d).getByText("Looks fine to me.")).toBeInTheDocument();
  });

  it("comment_posts_the_text", async () => {
    const { api, user } = setup();
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await user.type(within(d).getByLabelText("Comment"), "Thanks, one question.");
    await user.click(within(d).getByRole("button", { name: "Comment" }));
    await waitFor(() => expect(api.ghActions).toEqual([["demo-dev/orbit", 41, true, { kind: "comment", body: "Thanks, one question." }]]));
  });

  it("merge_asks_first", async () => {
    const { api, user } = setup();
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await user.click(within(d).getByRole("button", { name: "Merge" }));
    expect(within(d).getByText("Merge #41 into main?")).toBeInTheDocument();
    expect(api.ghActions).toEqual([]);
    await user.click(within(d).getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(api.ghActions).toEqual([["demo-dev/orbit", 41, true, { kind: "merge" }]]));
  });

  it("close_asks_first", async () => {
    const { api, user } = setup();
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await user.click(within(d).getByRole("button", { name: "Close" }));
    expect(within(d).getByText("Close #41?")).toBeInTheDocument();
    await user.click(within(d).getByRole("button", { name: "Cancel" }));
    expect(api.ghActions).toEqual([]);
  });

  it("the_agents_review_waits_as_a_draft", async () => {
    const { api, user } = setup(true);
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await user.click(within(d).getByRole("button", { name: "Review with the agent" }));
    const draft = await within(d).findByLabelText("Agent's review");
    expect((draft as HTMLTextAreaElement).value).toContain("the dot never turns grey");
    expect(api.ghActions).toEqual([]);
    await user.click(within(d).getByRole("button", { name: "Post as comment" }));
    await waitFor(() => expect(api.ghActions).toEqual([["demo-dev/orbit", 41, true, { kind: "comment", body: expect.stringContaining("the dot never turns grey") }]]));
  });

  it("a_posted_or_discarded_review_does_not_come_back", async () => {
    const { api, user } = setup(true);
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await user.click(within(d).getByRole("button", { name: "Review with the agent" }));
    await user.click(within(d).getByRole("button", { name: "Discard" }, await within(d).findByLabelText("Agent's review")));
    await new Promise((r) => setTimeout(r, 50));
    expect(within(d).queryByLabelText("Agent's review")).toBeNull();
    expect(within(d).queryByText(/The agent is reviewing/)).toBeNull();
    await user.click(within(d).getByRole("button", { name: "Review with the agent" }));
    await user.click(within(d).getByRole("button", { name: "Post as comment" }, await within(d).findByLabelText("Agent's review")));
    await waitFor(() => expect(api.ghActions).toHaveLength(1));
    await new Promise((r) => setTimeout(r, 50));
    expect(within(d).queryByLabelText("Agent's review")).toBeNull();
  });

  it("a_review_in_progress_survives_leaving_the_pull_request", async () => {
    const { api, user } = setup(true);
    let answer: string | null = null;
    vi.spyOn(api, "ghReviewDraft").mockImplementation(async () => answer);
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    await user.click(within(await detail()).getByRole("button", { name: "Review with the agent" }));
    await within(await detail()).findByText(/The agent is reviewing/);
    await user.click(within(await list()).getByRole("button", { name: /#40/ }));
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await screen.findByRole("region", { name: /^#41/ });
    expect(within(d).getByText(/The agent is reviewing/)).toBeInTheDocument();
    expect(within(d).getByRole("button", { name: "Review with the agent" })).toBeDisabled();
    answer = "Looks fine.";
    expect(((await within(d).findByLabelText("Agent's review", {}, { timeout: 12_000 })) as HTMLTextAreaElement).value).toBe("Looks fine.");
  }, 15_000);

  it("agent_buttons_explain_why_they_are_off", async () => {
    const { user } = setup(false);
    await user.click(within(await list()).getByRole("button", { name: /#41/ }));
    const d = await detail();
    await waitFor(() => expect(within(d).getByRole("button", { name: "Review with the agent" })).toBeDisabled());
    expect(within(d).getByText("Set the project's agent folder first.")).toBeInTheDocument();
  });

  it("new_issue_creates_it_in_the_project", async () => {
    const { api, user } = setup();
    await user.click(screen.getByRole("button", { name: "New issue" }));
    const dlg = screen.getByRole("dialog", { name: "New issue" });
    await user.selectOptions(within(dlg).getByLabelText("Project"), "orbit");
    await user.type(within(dlg).getByLabelText("Issue title"), "Dark mode toggle");
    await user.type(within(dlg).getByLabelText("Issue body"), "Please.");
    await user.click(within(dlg).getByRole("button", { name: "Create issue" }));
    await waitFor(() => expect(api.ghNewIssues).toEqual([["demo-dev/orbit", "Dark mode toggle", "Please."]]));
  });

  it("lists_reload_every_two_minutes", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { api } = setup();
    const spy = vi.spyOn(api, "ghList");
    await list();
    const before = spy.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(120_000);
    });
    await waitFor(() => expect(spy.mock.calls.length).toBe(before + 1));
  });
});
