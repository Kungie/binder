import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { type Screen, StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

function setup(initialScreen: Screen = "printer") {
  const api = memoryApi(demoSeed());
  const user = userEvent.setup();
  render(
    <StoreProvider api={api} initialScreen={initialScreen}>
      <Shell />
    </StoreProvider>,
  );
  return { api, user };
}

describe("Printer", () => {
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

  it("overview_shows_printing_printer", async () => {
    setup("overview");
    const panel = await screen.findByRole("region", { name: "Printer" });
    expect(within(panel).getByText("case-lid")).toBeInTheDocument();
    expect(within(panel).getByText("42%")).toBeInTheDocument();
  });
});
