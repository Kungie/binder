import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup() {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup({ advanceTimers: (ms) => void (vi.isFakeTimers() && vi.advanceTimersByTime(ms)) });
  render(
    <StoreProvider api={api} initialScreen="sites" updateDelay={60_000}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

afterEach(() => vi.useRealTimers());

describe("Sites", () => {
  it("the_sidebar_has_sites_under_server", () => {
    setup();
    const names = within(screen.getByRole("navigation", { name: "Main" })).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent);
    const server = names.findIndex((n) => n?.startsWith("Server"));
    expect(names[server + 1]).toMatch(/^Sites/);
  });

  it("each_site_shows_preview_dot_and_figures", async () => {
    setup();
    const card = await screen.findByRole("article", { name: "blog" });
    const frame = await within(card).findByTitle("blog preview");
    // Every preview is a still page without scripts, even for a site that allows frames: live pages made the
    // screen stutter.
    expect(frame).not.toHaveAttribute("src");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(within(card).getByRole("link", { name: "blog.example.com" })).toBeInTheDocument();
    await waitFor(() => expect(within(card).getByRole("img", { name: "Up" })).toBeInTheDocument());
    expect(within(card).getByText("CPU 0.4% · RAM 38 MB · Disk 12 MB")).toBeInTheDocument();
  });

  it("a_site_that_cant_be_framed_shows_a_snapshot_without_scripts", async () => {
    setup();
    const card = await screen.findByRole("article", { name: "shop" });
    const frame = await within(card).findByTitle("shop preview");
    expect(frame).toHaveAttribute("sandbox", "");
    expect(frame).not.toHaveAttribute("src");
    expect(frame.getAttribute("srcdoc")).toContain("<base href=");
  });

  it("refresh_reads_the_previews_again", async () => {
    const { api, user } = setup();
    const spy = vi.spyOn(api, "siteSnapshot");
    const card = await screen.findByRole("article", { name: "blog" });
    await within(card).findByTitle("blog preview");
    const before = spy.mock.calls.filter(([u]) => u === "https://blog.example.com").length;
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(spy.mock.calls.filter(([u]) => u === "https://blog.example.com").length).toBe(before + 1));
  });

  it("a_site_whose_snapshot_fails_shows_its_name", async () => {
    const { api } = setup();
    vi.spyOn(api, "siteSnapshot").mockRejectedValue(new Error("No answer"));
    const card = await screen.findByRole("article", { name: "shop" });
    await waitFor(() => expect(within(card).queryByTitle("shop preview")).toBeNull());
    expect(within(card).getByText("S")).toBeInTheDocument();
  });

  it("a_down_site_says_why", async () => {
    setup();
    const card = await screen.findByRole("article", { name: "shop" });
    const dot = await within(card).findByRole("img", { name: "Down" });
    expect(dot).toHaveAttribute("title", "shop-site-1 is exited");
  });

  it("add_address_saves_it", async () => {
    const { api, user } = setup();
    const card = await screen.findByRole("article", { name: "notes" });
    await user.click(within(card).getByRole("button", { name: "Add address" }));
    await user.type(within(card).getByLabelText("Address"), "notes.example.com{Enter}");
    await waitFor(async () => expect((await api.getConfig()).site_addresses).toEqual({ notes: "https://notes.example.com" }));
    expect(await within(card).findByRole("link", { name: "notes.example.com" })).toBeInTheDocument();
  });

  it("figures_refresh_every_30_seconds", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { api } = setup();
    const spy = vi.spyOn(api, "sites");
    await screen.findByRole("article", { name: "blog" });
    const before = spy.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(30_000);
    });
    await waitFor(() => expect(spy.mock.calls.length).toBe(before + 1));
  });
});
