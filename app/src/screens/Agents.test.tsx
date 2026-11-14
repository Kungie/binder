import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect, useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { type MemoryOptions, demoSeed, memoryApi } from "../lib/memory";

/** Stands in for xterm.js: a textarea for keys, and the written bytes as text. */
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
    <StoreProvider api={api}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

async function openAgents(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Agents" }));
}

describe("Agents", () => {
  it("shows both agents with their sessions", async () => {
    const { user } = setup();
    await openAgents(user);
    expect(await screen.findByRole("region", { name: "Claude Code" })).toHaveTextContent("binder-claude");
    expect(screen.getByRole("region", { name: "Qwen3 4B" })).toHaveTextContent("binder-qwen");
    expect(await screen.findAllByText(/Connected to demo-dev@10.0.7.2/)).toHaveLength(2);
  });

  it("asks for a server when none is set", async () => {
    const { user } = setup({ server: null });
    await openAgents(user);
    expect(await screen.findByText("Set the home server in Settings to use Agents.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Claude Code" })).toBeNull();
  });

  it("a failed or ended session shows the message and Reconnect", async () => {
    const { user } = setup({ termFail: "tmux isn't installed on the server." });
    await openAgents(user);
    const left = await screen.findByRole("region", { name: "Claude Code" });
    expect(await within(left).findByText("tmux isn't installed on the server.")).toBeInTheDocument();
    expect(within(left).getByRole("button", { name: "Reconnect" })).toBeInTheDocument();
  });

  it("typed keys reach the session", async () => {
    const { user } = setup();
    await openAgents(user);
    const left = await screen.findByRole("region", { name: "Claude Code" });
    await within(left).findByText(/Connected to/);
    await user.type(within(left).getByLabelText("Terminal input"), "x");
    expect(await within(left).findByText(/\(demo\)\s*x/)).toBeInTheDocument();
  });

  it("terminal_keeps_keys_but_palette_opens", async () => {
    const { user } = setup();
    await openAgents(user);
    const input = within(await screen.findByRole("region", { name: "Claude Code" })).getByLabelText("Terminal input");
    await user.click(input);
    await user.keyboard("c");
    expect(screen.queryByRole("dialog", { name: "New task" })).toBeNull();
    await user.keyboard("{Meta>}k{/Meta}");
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("cmd-2 focuses the right terminal", async () => {
    const { user } = setup();
    await openAgents(user);
    const right = await screen.findByRole("region", { name: "Qwen3 4B" });
    await user.keyboard("{Meta>}2{/Meta}");
    expect(within(right).getByLabelText("Terminal input")).toHaveFocus();
  });

  it("the terminals stay connected across screens", async () => {
    const { api, user } = setup();
    const open = vi.spyOn(api, "termOpen");
    await openAgents(user);
    await screen.findAllByText(/Connected to demo-dev/);
    await user.click(screen.getByRole("button", { name: "Server" }));
    await openAgents(user);
    expect(screen.getAllByText(/Connected to demo-dev/)).toHaveLength(2);
    expect(open).toHaveBeenCalledTimes(2);
  });
});
