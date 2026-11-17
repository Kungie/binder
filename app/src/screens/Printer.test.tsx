import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { type Screen, StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup(initialScreen: Screen = "printer", seed = demoSeed()) {
  const api = memoryApi(seed);
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Printer", () => {
  it("the_sidebar_says_3d_printer", async () => {
    setup("overview");
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).getByRole("button", { name: /^3D Printer/ })).toBeInTheDocument();
    expect(within(nav).queryByRole("button", { name: /^Printer/ })).toBeNull();
  });

  it("the_screen_heading_says_3d_printer", async () => {
    setup();
    expect(await screen.findByRole("heading", { level: 1, name: "3D Printer" })).toBeInTheDocument();
  });

  it("overview_panel_keeps_printer", async () => {
    setup("overview");
    expect(await screen.findByRole("region", { name: "Printer" })).toBeInTheDocument();
  });

  it("printer_shows_progress_and_temps", async () => {
    setup();
    const panel = await screen.findByRole("region", { name: "workshop" });
    expect(within(panel).getByText("case-lid")).toBeInTheDocument();
    expect(within(panel).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "42");
    expect(within(panel).getByText("120 of 300")).toBeInTheDocument();
    expect(within(panel).getByText("1 h 13 min")).toBeInTheDocument();
    expect(within(panel).getByText("220 / 220°C")).toBeInTheDocument();
    expect(within(panel).getByText("55 / 55°C")).toBeInTheDocument();
  });

  it("stop_asks_first", async () => {
    const { api, user } = setup();
    const panel = await screen.findByRole("region", { name: "workshop" });
    await user.click(within(panel).getByRole("button", { name: "Stop" }));
    expect(within(panel).getByText("Stop the print? It can't be resumed.")).toBeInTheDocument();
    expect(api.printerCommands).toEqual([]);
    await user.click(within(panel).getByRole("button", { name: "Stop print" }));
    await waitFor(() => expect(api.printerCommands).toEqual([["DEMO0SERIAL01", "stop"]]));
  });

  it("access_code_field_is_password_and_not_echoed", async () => {
    const { api, user } = setup();
    await user.click(await screen.findByRole("button", { name: "Add printer" }));
    const dialog = screen.getByRole("dialog", { name: "Add printer" });
    const code = within(dialog).getByLabelText("Access code");
    expect(code).toHaveAttribute("type", "password");
    expect(code).toHaveAttribute("autocomplete", "off");
    await user.type(within(dialog).getByLabelText("Name"), "spare");
    await user.type(within(dialog).getByLabelText("Address"), "10.0.7.31");
    await user.type(within(dialog).getByLabelText("Serial"), "DEMO0SERIAL02");
    await user.type(code, "12345678");
    await user.click(within(dialog).getByRole("button", { name: "Add printer" }));
    expect(await screen.findByRole("region", { name: "spare" })).toBeInTheDocument();
    expect(document.body.textContent).not.toContain("12345678");
    expect(JSON.stringify(await api.printers())).not.toContain("12345678");
  });

  it("overview_shows_an_idle_printer", async () => {
    const seed = demoSeed();
    seed.printers![0].state = { ...seed.printers![0].state, gcode_state: "IDLE", percent: null, job: null, nozzle_c: 24.6, nozzle_target_c: 0, bed_c: 23.1, bed_target_c: 0 };
    setup("overview", seed);
    const panel = await screen.findByRole("region", { name: "Printer" });
    expect(within(panel).getByText("workshop")).toBeInTheDocument();
    expect(within(panel).getByText("Idle")).toBeInTheDocument();
    expect(within(panel).getByText("25 / 0°C")).toBeInTheDocument();
    expect(within(panel).queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("overview_shows_an_offline_printer", async () => {
    const seed = demoSeed();
    seed.printers![0].online = false;
    seed.printers![0].state = { ...seed.printers![0].state, gcode_state: null };
    setup("overview", seed);
    const panel = await screen.findByRole("region", { name: "Printer" });
    expect(within(panel).getByText("Offline")).toBeInTheDocument();
  });

  it("overview_says_when_no_printer_is_set_up", async () => {
    const seed = demoSeed();
    seed.printers = [];
    const { user } = setup("overview", seed);
    const panel = await screen.findByRole("region", { name: "Printer" });
    expect(within(panel).getByText("No printer set up.")).toBeInTheDocument();
    await user.click(within(panel).getByRole("button", { name: "Set up" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Overview" })).not.toBeInTheDocument());
  });

  it("overview_shows_printing_printer", async () => {
    setup("overview");
    const panel = await screen.findByRole("region", { name: "Printer" });
    expect(within(panel).getByText("case-lid")).toBeInTheDocument();
    expect(within(panel).getByText("42%")).toBeInTheDocument();
  });

  it("problems_show_in_plain_words", async () => {
    const seed = demoSeed();
    seed.printers![0].problems = [{ code: "0700200000020001", text: "AMS A Slot 1 filament has run out." }];
    setup("printer", seed);
    const panel = await screen.findByRole("region", { name: "workshop" });
    const list = within(panel).getByRole("list", { name: "Printer messages" });
    expect(within(list).getByText("AMS A Slot 1 filament has run out.")).toBeInTheDocument();
    expect(within(list).getByText("0700-2000-0002-0001")).toBeInTheDocument();
  });

  it("light_and_fan_send_their_commands", async () => {
    const seed = demoSeed();
    seed.printers![0].state.light_on = false;
    seed.printers![0].state.fan_percent = 0;
    const { api, user } = setup("printer", seed);
    const panel = await screen.findByRole("region", { name: "workshop" });
    await user.click(within(panel).getByRole("switch", { name: "Light" }));
    await user.selectOptions(within(panel).getByLabelText("Part fan"), "50");
    expect(api.printerCommands).toEqual([
      ["DEMO0SERIAL01", { light: { on: true } }],
      ["DEMO0SERIAL01", { fan: { percent: 50 } }],
    ]);
  });

  it("history_lists_ended_prints", async () => {
    const { user } = setup();
    const panel = await screen.findByRole("region", { name: "Print history" });
    const rows = within(panel).getAllByRole("listitem");
    expect(rows[0]).toHaveTextContent("bracket");
    expect(rows[0]).toHaveTextContent("Failed at layer 12");
    expect(rows[1]).toHaveTextContent("case-lid");
    expect(rows[1]).toHaveTextContent("1 h 35 min");
    void user;
  });

  it("the_camera_shows_its_frames_and_stops_when_hidden", async () => {
    const seed = demoSeed();
    const { api, user } = setup("printer", seed);
    const urls: string[] = [];
    globalThis.URL.createObjectURL = (b: Blob) => {
      urls.push(`blob:${b.size}`);
      return `blob:frame-${urls.length}`;
    };
    globalThis.URL.revokeObjectURL = () => {};
    const panel = await screen.findByRole("region", { name: "workshop" });
    await user.click(within(panel).getByRole("button", { name: "Show camera" }));
    const img = await within(panel).findByRole("img", { name: "workshop camera" });
    await waitFor(() => expect(img.getAttribute("src")).toMatch(/^blob:frame-/));
    await user.click(within(panel).getByRole("button", { name: "Hide camera" }));
    expect(api.cameraStops).toEqual(["DEMO0SERIAL01"]);
  });
});
