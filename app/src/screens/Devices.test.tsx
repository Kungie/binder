import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect, useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";

vi.mock("../lib/pick", () => ({ pickFiles: async () => ["/Users/demo/a.txt"], pickFolder: async () => null, pickApp: async () => null }));

function FakeXterm({ onData, register }: { onData(data: string): void; onResize(cols: number, rows: number): void; register(write: (bytes: Uint8Array) => void, focus: () => void): void }) {
  const [text, setText] = useState("");
  const input = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const decoder = new TextDecoder();
    register(
      (bytes) => setText((t) => t + decoder.decode(bytes)),
      () => input.current?.focus(),
    );
  }, [register]);
  return (
    <div>
      <textarea aria-label="Terminal input" ref={input} value="" onChange={(e) => onData(e.target.value)} />
      <pre>{text}</pre>
    </div>
  );
}

vi.mock("../components/XtermView", () => ({ XtermView: FakeXterm }));

function setup(options: MemoryOptions = {}) {
  const api = memoryApi(demoSeed(), options);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen="devices">
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Devices", () => {
  it("devices_add_and_show_online", async () => {
    const api = memoryApi(demoSeed());
    const user = userEvent.setup();
    render(
      <StoreProvider api={api} initialScreen="devices">
        <Shell />
      </StoreProvider>,
    );
    const table = await screen.findByRole("table", { name: "Devices" });
    await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(4));
    await waitFor(() => expect(within(table).getByRole("row", { name: /printer/ })).toHaveTextContent("Offline"));

    await user.click(screen.getByRole("button", { name: "New device" }));
    const dialog = screen.getByRole("dialog", { name: "New device" });
    await user.type(within(dialog).getByLabelText("Name"), "nas");
    await user.type(within(dialog).getByLabelText("Address"), "10.0.7.40");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(within(table).getByRole("row", { name: /nas/ })).toHaveTextContent("Online"));
    expect(screen.queryByRole("dialog", { name: "New device" })).toBeNull();

    await user.click(within(table).getByRole("row", { name: /nas/ }));
    const note = await screen.findByLabelText("Note");
    await user.type(note, "In the closet");
    await user.keyboard("{Escape}");
    await waitFor(async () => expect((await api.listDevices()).find((d) => d.name === "nas")?.note).toBe("In the closet"));
  });

  it("lists the tailnet with state, route and keys", async () => {
    setup();
    const panel = await screen.findByRole("region", { name: "Tailscale" });
    const lab = await within(panel).findByRole("row", { name: /lab/ });
    expect(lab).toHaveTextContent("Online");
    expect(await within(lab).findByText("31 ms · DERP fra")).toBeInTheDocument();
    expect(within(lab).getByLabelText("Ping, last 24 hours")).toBeInTheDocument();
    const phone = within(panel).getByRole("row", { name: /phone/ });
    expect(phone).toHaveTextContent("Offline");
    expect(phone).toHaveTextContent("3 days ago");
    expect(within(phone).getByText("Key expired")).toHaveClass("text-bad");
    expect(within(panel).getByRole("row", { name: /demo-mac/ })).toHaveTextContent("This Mac");
  });

  it("says when Tailscale isn't installed", async () => {
    setup({ tailnet: null });
    expect(await screen.findByText("Tailscale isn't installed on this Mac.")).toBeInTheDocument();
  });

  it("sends a file with Taildrop", async () => {
    const { api, user } = setup();
    const panel = await screen.findByRole("region", { name: "Tailscale" });
    const lab = await within(panel).findByRole("row", { name: /lab/ });
    await user.click(within(lab).getByRole("button", { name: "Send file" }));
    expect(await screen.findByText("Sent a.txt to lab.")).toBeInTheDocument();
    expect(api.dropped).toEqual([{ target: "lab.tail0.ts.net", paths: ["/Users/demo/a.txt"] }]);
  });

  it("opens an SSH shell on a device and closes it", async () => {
    const { api, user } = setup({ server: "demo-dev@10.0.7.2" });
    const open = vi.spyOn(api, "termOpen");
    const close = vi.spyOn(api, "termClose");
    const panel = await screen.findByRole("region", { name: "Tailscale" });
    await user.click(within(await within(panel).findByRole("row", { name: /lab/ })).getByRole("button", { name: "SSH" }));
    const field = screen.getByRole("textbox", { name: "SSH target" });
    expect(field).toHaveValue("demo-dev@lab.tail0.ts.net");
    await user.click(screen.getByRole("button", { name: "Connect" }));
    const term = await screen.findByRole("region", { name: "SSH · lab" });
    expect(open).toHaveBeenLastCalledWith({ kind: "ssh", target: "demo-dev@lab.tail0.ts.net" }, expect.any(Number), expect.any(Number), expect.any(Function));
    await user.click(within(term).getByRole("button", { name: "Close" }));
    expect(close).toHaveBeenCalledWith("ssh-demo-dev-lab-tail0-ts-net");
    expect(screen.queryByRole("region", { name: "SSH · lab" })).toBeNull();
  });

  it("marks the exit node in use", async () => {
    setup();
    const panel = await screen.findByRole("region", { name: "Tailscale" });
    const vps = await within(panel).findByRole("row", { name: /^vps / });
    expect(within(vps).getByText("Exit node")).toBeInTheDocument();
    expect(within(panel).getByRole("row", { name: /lab/ })).not.toHaveTextContent("Exit node");
  });

  it("a device on the tailnet takes its state from Tailscale", async () => {
    const { api, user } = setup();
    await api.createDevice({ name: "iPhone", kind: "other", address: "phone" });
    await user.click(await screen.findByRole("button", { name: /^Tasks/ }));
    await user.click(screen.getByRole("button", { name: "Devices" }));
    const table = await screen.findByRole("table", { name: "Devices" });
    const row = await within(table).findByRole("row", { name: /iPhone/ });
    await waitFor(() => expect(row).toHaveTextContent("Offline"));
    expect(row).toHaveTextContent("Tailscale");
  });
});
