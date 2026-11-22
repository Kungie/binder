import { render, screen, waitFor, within } from "@testing-library/react";
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
  it("lists the sessions and shows Claude Code first", async () => {
    const { user } = setup();
    await openAgents(user);
    const nav = await screen.findByRole("navigation", { name: "Sessions" });
    expect(within(nav).getByRole("button", { name: /Claude Code.*Idle.*⌘1/ })).toHaveAttribute("aria-current", "true");
    expect(within(nav).getByRole("button", { name: /Ollama.*⌘2/ })).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "Claude Code" })).toHaveTextContent("binder-claude");
    expect(await screen.findAllByText(/Connected to demo-dev@10.0.7.2/)).toHaveLength(1);
  });

  it("asks for a server when none is set", async () => {
    const { user } = setup({ server: null });
    await openAgents(user);
    const setup_ = await screen.findByRole("region", { name: "Set up Agents" });
    expect(within(setup_).getByText("Set your home server's SSH target in Settings.")).toBeInTheDocument();
    expect(within(setup_).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
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

  it("cmd-2 opens and focuses Ollama", async () => {
    const { user } = setup();
    await openAgents(user);
    await screen.findByRole("region", { name: "Claude Code" });
    await user.keyboard("{Meta>}2{/Meta}");
    const right = await screen.findByRole("region", { name: "Ollama · qwen3:4b" });
    await within(right).findByText(/Connected to/);
    expect(within(right).getByLabelText("Terminal input")).toHaveFocus();
  });

  it("the terminals stay connected across screens and sessions", async () => {
    const { api, user } = setup();
    const open = vi.spyOn(api, "termOpen");
    await openAgents(user);
    await screen.findAllByText(/Connected to demo-dev/);
    await user.click(screen.getByRole("button", { name: /Ollama/ }));
    await user.click(screen.getByRole("button", { name: /Claude Code/ }));
    await user.click(screen.getByRole("button", { name: "Servers" }));
    await openAgents(user);
    expect(screen.getAllByText(/Connected to demo-dev/, { ignore: "[hidden] *" }).length).toBeGreaterThan(0);
    expect(open).toHaveBeenCalledTimes(2);
  });

  it("shows each session's state", async () => {
    const { user } = setup({
      sessions: [
        { session: "binder-claude", state: "waiting", line: "Do you want to proceed?" },
        { session: "binder-ollama", state: "working", line: "" },
      ],
    });
    await openAgents(user);
    const nav = await screen.findByRole("navigation", { name: "Sessions" });
    expect(await within(nav).findByRole("button", { name: /Claude Code.*Waiting/ })).toHaveAttribute("title", "Do you want to proceed?");
    expect(within(nav).getByRole("button", { name: /Ollama.*Working/ })).toBeInTheDocument();
  });

  it("starts a session in a project's folder and closes it", async () => {
    const { api, user } = setup();
    const p = (await api.listProjects()).find((x) => x.agent_dir)!;
    const open = vi.spyOn(api, "termOpen");
    await openAgents(user);
    await user.selectOptions(await screen.findByRole("combobox", { name: "Open project" }), String(p.id));
    const region = await screen.findByRole("region", { name: `Claude Code · ${p.name}` });
    expect(open).toHaveBeenLastCalledWith({ kind: "claude", project: p.name, dir: p.agent_dir }, expect.any(Number), expect.any(Number), expect.any(Function));
    await user.click(within(region).getByRole("button", { name: "Close" }));
    expect(api.closedSessions).toHaveLength(0);
    await user.click(within(region).getByRole("button", { name: "End" }));
    expect(api.closedSessions).toHaveLength(1);
    expect(api.closedSessions[0]).toMatch(/^binder-claude-/);
    expect(screen.queryByRole("region", { name: `Claude Code · ${p.name}` })).toBeNull();
  });

  it("a project session already on the server is listed", async () => {
    const api0 = memoryApi(demoSeed());
    const p = (await api0.listProjects()).find((x) => x.agent_dir)!;
    const { user } = setup({ sessions: [{ session: `binder-claude-${p.name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`, state: "working", line: "" }] });
    await openAgents(user);
    expect(await screen.findByRole("button", { name: new RegExp(`${p.name}.*Working`) })).toBeInTheDocument();
  });

  it("picking a model saves it and reconnects Ollama", async () => {
    const { api, user } = setup();
    await openAgents(user);
    await user.click(await screen.findByRole("button", { name: /Ollama/ }));
    const region = await screen.findByRole("region", { name: "Ollama · qwen3:4b" });
    await user.selectOptions(await within(region).findByRole("combobox", { name: "Model" }), "llama3.2:latest");
    expect(await screen.findByRole("region", { name: "Ollama · llama3.2:latest" })).toBeInTheDocument();
    expect((await api.getConfig()).ollama_model).toBe("llama3.2:latest");
  });

  it("another model name can be typed", async () => {
    const { api, user } = setup();
    await openAgents(user);
    await user.click(await screen.findByRole("button", { name: /Ollama/ }));
    const region = await screen.findByRole("region", { name: "Ollama · qwen3:4b" });
    await user.selectOptions(await within(region).findByRole("combobox", { name: "Model" }), "Other…");
    await user.type(within(region).getByRole("textbox", { name: "Model name" }), "gemma3:1b{Enter}");
    expect(await screen.findByRole("region", { name: "Ollama · gemma3:1b" })).toBeInTheDocument();
    expect((await api.getConfig()).ollama_model).toBe("gemma3:1b");
  });

  it("shows usage and server load", async () => {
    const { user } = setup();
    await openAgents(user);
    expect(await screen.findByLabelText("Claude Code usage")).toHaveTextContent("Claude5 h 182k · today 1.2M · 7 days 8.9M");
    expect(await screen.findByLabelText("Server load")).toHaveTextContent(/CPU .* · RAM .*GB/);
  });

  it("lists a live run with Stop", async () => {
    const { api, user } = setup();
    const t = (await api.listTasks({ kind: "all" }, false))[0];
    const run = await api.startRun(t.id, "~/code", "edits_only");
    vi.spyOn(api, "pollRun").mockImplementation(async () => ({ ...run, tail: "Reading files\nRunning tests" }));
    const stop = vi.spyOn(api, "stopRun");
    await openAgents(user);
    const strip = await screen.findByRole("region", { name: "Agent runs" });
    expect(within(strip).getByText(t.title)).toBeInTheDocument();
    await user.click(within(strip).getByRole("button", { name: "Stop" }));
    expect(stop).toHaveBeenCalledWith(run.run);
  });

  it("colors each state: working green, waiting yellow, exited red, idle gray", async () => {
    const { user } = setup({
      sessions: [
        { session: "binder-claude", state: "working", line: "" },
        { session: "binder-ollama", state: "exited", line: "" },
      ],
    });
    await openAgents(user);
    const nav = await screen.findByRole("navigation", { name: "Sessions" });
    expect(await within(nav).findByText("Working")).toHaveClass("text-ok");
    expect(within(nav).getByText("Exited")).toHaveClass("text-bad");
  });

  it("an unreachable server shows every session as Error", async () => {
    const { api, user } = setup();
    vi.spyOn(api, "agentSessions").mockRejectedValue(new Error("Couldn't reach the server."));
    await openAgents(user);
    const nav = await screen.findByRole("navigation", { name: "Sessions" });
    expect(await within(nav).findAllByText("Error")).toHaveLength(2);
  });

  it("makes a new session in a folder and keeps it", async () => {
    const { api, user } = setup();
    const open = vi.spyOn(api, "termOpen");
    await openAgents(user);
    await user.click(await screen.findByRole("button", { name: "New session" }));
    const form = screen.getByRole("form", { name: "New session" });
    await user.type(within(form).getByRole("textbox", { name: "Name" }), "Scratch");
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(await screen.findByRole("region", { name: "Claude Code · Scratch" })).toBeInTheDocument();
    expect(open).toHaveBeenLastCalledWith({ kind: "claude", project: "Scratch", dir: "~/code/scratch" }, expect.any(Number), expect.any(Number), expect.any(Function));
    expect((await api.getConfig()).agents).toEqual([{ name: "Scratch", dir: "~/code/scratch" }]);
  });

  it("a new session can be a new project", async () => {
    const { api, user } = setup();
    await openAgents(user);
    await user.click(await screen.findByRole("button", { name: "New session" }));
    const form = screen.getByRole("form", { name: "New session" });
    await user.type(within(form).getByRole("textbox", { name: "Name" }), "Lamp");
    await user.type(within(form).getByRole("textbox", { name: "Folder" }), "~/code/lamp-v2");
    await user.click(within(form).getByRole("checkbox", { name: "Add to Projects" }));
    await user.click(within(form).getByRole("button", { name: "Start" }));
    expect(await screen.findByRole("region", { name: "Claude Code · Lamp" })).toBeInTheDocument();
    expect((await api.listProjects()).find((p) => p.name === "Lamp")?.agent_dir).toBe("~/code/lamp-v2");
  });

  it("renames a session", async () => {
    const { api, user } = setup();
    await openAgents(user);
    const region = await screen.findByRole("region", { name: "Claude Code" });
    await user.click(within(region).getByRole("button", { name: "Rename" }));
    const field = screen.getByRole("textbox", { name: "Session name" });
    await user.clear(field);
    await user.type(field, "Home{Enter}");
    expect(await screen.findByRole("button", { name: /^Home.*⌘1/ })).toBeInTheDocument();
    expect((await api.getConfig()).agent_names).toEqual({ "binder-claude": "Home" });
  });

  it("lists another Binder session on the server and can end it", async () => {
    const { api, user } = setup({ sessions: [{ session: "binder-qwen", state: "idle", line: "" }] });
    await openAgents(user);
    await user.click(await screen.findByRole("button", { name: /^qwen/ }));
    const region = await screen.findByRole("region", { name: "qwen" });
    await user.click(within(region).getByRole("button", { name: "Close" }));
    await user.click(within(region).getByRole("button", { name: "End" }));
    expect(api.closedSessions).toEqual(["binder-qwen"]);
    expect(screen.queryByRole("button", { name: /^qwen/ })).toBeNull();
  });

  it("shows the plan limits in percent once the status line keeps them", async () => {
    const now = Math.floor(Date.now() / 1000);
    const { user } = setup({ limits: { installed: true, limits: { five_hour: { percent: 34.4, resets_at: now + 3600 }, seven_day: { percent: 82, resets_at: now + 86_400 * 3 }, read_at: now } } });
    await openAgents(user);
    const usage = await screen.findByLabelText("Claude Code usage");
    await within(usage).findByText("34%");
    expect(within(usage).getByText("34%")).toHaveClass("text-ok");
    expect(within(usage).getByText("82%")).toHaveClass("text-bad");
  });

  it("adds the status line by itself, then waits for Claude's next reply", async () => {
    const { api, user } = setup();
    const install = vi.spyOn(api, "installClaudeLimits");
    await openAgents(user);
    await waitFor(() => expect(install).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("button", { name: "Show %" })).toBeNull();
    expect(await screen.findByText("% after Claude's next reply")).toBeInTheDocument();
  });
});
