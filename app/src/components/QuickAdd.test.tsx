import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { QuickAdd } from "./QuickAdd";
import { demoSeed, memoryApi } from "../lib/memory";

describe("Quick add window", () => {
  it("enter_makes_the_task_and_shows_what_it_read", async () => {
    const api = memoryApi(demoSeed());
    const added = vi.spyOn(api, "quickAdded");
    const user = userEvent.setup();
    render(<QuickAdd api={api} />);
    const field = await screen.findByRole("textbox", { name: "New task" });
    await user.type(field, "Pay rent every month #orbit");
    expect(screen.getByText("Monthly")).toBeInTheDocument();
    expect(screen.getByText("orbit")).toBeInTheDocument();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(added).toHaveBeenCalled());
    const t = (await api.listTasks({ kind: "all" }, true)).find((x) => x.title === "Pay rent");
    expect(t).toMatchObject({ repeat: "monthly" });
    expect((field as HTMLInputElement).value).toBe("");
  });

  it("escape_closes_without_a_task", async () => {
    const api = memoryApi(demoSeed());
    const hide = vi.spyOn(api, "quickAdded");
    const close = vi.spyOn(api, "quickClose");
    const user = userEvent.setup();
    render(<QuickAdd api={api} />);
    await user.type(await screen.findByRole("textbox", { name: "New task" }), "Nothing{Escape}");
    expect(close).toHaveBeenCalled();
    expect(hide).not.toHaveBeenCalled();
  });
});
