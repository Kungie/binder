import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { type MemoryOptions, demoSeed, memoryApi } from "./memory";
import { StoreProvider } from "./store";

function setup(options: MemoryOptions = {}) {
  const api = memoryApi(demoSeed(), options);
  const check = vi.spyOn(api, "checkUpdate");
  render(
    <StoreProvider api={api} updateDelay={0}>
      <Shell />
    </StoreProvider>,
  );
  return { api, check };
}

describe("the update check at start", () => {
  it("a newer release at start puts a dot beside Settings", async () => {
    setup({ update: { version: "2.5.0", notes: "" } });
    expect(await screen.findByRole("img", { name: "Update available" })).toBeInTheDocument();
  });

  it("no dot when Binder is up to date", async () => {
    const { check } = setup();
    await waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    expect(screen.queryByRole("img", { name: "Update available" })).toBeNull();
  });

  it("a failed check at start shows nothing", async () => {
    const { check } = setup({ updateFail: "check" });
    await waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    expect(screen.queryByRole("img", { name: "Update available" })).toBeNull();
    expect(screen.queryByText("Couldn't check for updates.")).toBeNull();
  });

  it("development builds never check", async () => {
    const { check } = setup({ dev: true });
    await new Promise((r) => setTimeout(r, 50));
    expect(check).not.toHaveBeenCalled();
  });
});
