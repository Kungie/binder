import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";
import { StoreProvider } from "../lib/store";

function setup(options: MemoryOptions = {}, checkTimeout?: number) {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="settings" updateDelay={60_000} checkTimeout={checkTimeout}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

const check = (user: ReturnType<typeof userEvent.setup>) => user.click(screen.getByRole("button", { name: "Check for updates" }));

describe("Settings › Binder", () => {
  it("shows the running version", async () => {
    setup();
    expect(await screen.findByText("Version 2.4.0")).toBeInTheDocument();
  });

  it("up to date", async () => {
    const { user } = setup();
    await screen.findByText("Version 2.4.0");
    await check(user);
    expect(await screen.findByText("Binder is up to date.")).toBeInTheDocument();
  });

  it("a newer release shows its notes and the install button", async () => {
    const { user } = setup({ update: { version: "2.5.0", notes: "## New\n- Updates" } });
    await screen.findByText("Version 2.4.0");
    await check(user);
    expect(await screen.findByText("Binder 2.5.0 is available.")).toBeInTheDocument();
    expect(screen.getByText(/New\s+Updates/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeEnabled();
  });

  it("a failed check", async () => {
    const { user } = setup({ updateFail: "check" });
    await screen.findByText("Version 2.4.0");
    await check(user);
    expect(await screen.findByText("Couldn't check for updates.")).toBeInTheDocument();
  });

  it("installing shows progress and installs", async () => {
    let open = () => {};
    const installGate = () => new Promise<void>((r) => (open = r));
    const { api, user } = setup({ update: { version: "2.5.0", notes: "" }, installGate });
    await screen.findByText("Version 2.4.0");
    await check(user);
    await user.click(await screen.findByRole("button", { name: "Install and restart" }));
    expect(await screen.findByRole("button", { name: "Downloading 42%" })).toBeDisabled();
    await act(async () => open());
    expect(await screen.findByRole("button", { name: "Installing…" })).toBeDisabled();
    await act(async () => open());
    expect(api.installed).toBe(true);
  });

  it("a failed install shows the message and the button again", async () => {
    const { user } = setup({ update: { version: "2.5.0", notes: "" }, updateFail: "install" });
    await screen.findByText("Version 2.4.0");
    await check(user);
    await user.click(await screen.findByRole("button", { name: "Install and restart" }));
    expect(await screen.findByText("The update couldn't be installed.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeEnabled();
  });

  it("install waits for a cloud transfer", async () => {
    const { api, user } = setup({ update: { version: "2.5.0", notes: "" } });
    await screen.findByText("Version 2.4.0");
    await check(user);
    await screen.findByRole("button", { name: "Install and restart" });
    act(() => api.emitTransfer({ id: 7, name: "big.mov", unit: "bytes", done: 1, total: 10, state: "running", error: null }));
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeDisabled();
    expect(screen.getByText("Wait for the Cloud transfer to finish.")).toBeInTheDocument();
    act(() => api.emitTransfer({ id: 7, name: "big.mov", unit: "bytes", done: 10, total: 10, state: "done", error: null }));
    expect(screen.getByRole("button", { name: "Install and restart" })).toBeEnabled();
  });

  it("leaving Settings keeps the install going", async () => {
    const installGate = () => new Promise<void>(() => {});
    const { user } = setup({ update: { version: "2.5.0", notes: "" }, installGate });
    await screen.findByText("Version 2.4.0");
    await check(user);
    await user.click(await screen.findByRole("button", { name: "Install and restart" }));
    await screen.findByRole("button", { name: "Downloading 42%" });
    await user.click(screen.getByRole("button", { name: "Overview" }));
    await user.click(screen.getByRole("button", { name: /^Settings/ }));
    expect(await screen.findByRole("button", { name: "Downloading 42%" })).toBeDisabled();
  });

  it("a copy started during the download shows the wait line", async () => {
    const installGate = () => new Promise<void>(() => {});
    const { api, user } = setup({ update: { version: "2.5.0", notes: "" }, installGate });
    await screen.findByText("Version 2.4.0");
    await check(user);
    await user.click(await screen.findByRole("button", { name: "Install and restart" }));
    await screen.findByRole("button", { name: "Downloading 42%" });
    act(() => api.emitTransfer({ id: 8, name: "big.mov", unit: "bytes", done: 1, total: 10, state: "running", error: null }));
    expect(screen.getByText("Wait for the Cloud transfer to finish.")).toBeInTheDocument();
  });

  it("a check that hangs gives up", async () => {
    const { user } = setup({ updateHang: true }, 50);
    await screen.findByText("Version 2.4.0");
    await check(user);
    expect(await screen.findByText("Couldn't check for updates.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeEnabled();
  });

  it("development builds show no button", async () => {
    setup({ dev: true });
    expect(await screen.findByText("Updates are off in development builds.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
  });
});
