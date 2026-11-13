import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";

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
});
