import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Shell } from "../App";
import { colorFor } from "../lib/colors";
import { StoreProvider } from "../lib/store";
import { demoSeed, memoryApi } from "../lib/memory";
import { Chip } from "./Chip";
import { StatusIcon } from "./StatusIcon";

describe("Look", () => {
  it("status_icon_uses_status_colour", () => {
    render(<StatusIcon status="in_progress" />);
    expect(screen.getByRole("img", { name: "In progress" })).toHaveClass("text-st-progress");
  });

  it("chip_warn_tone_is_orange", () => {
    render(<Chip tone="warn">Yesterday</Chip>);
    expect(screen.getByText("Yesterday").closest("[data-chip]")).toHaveClass("text-urgent");
  });

  it("sidebar_shows_project_colour", async () => {
    render(
      <StoreProvider api={memoryApi(demoSeed())}>
        <Shell />
      </StoreProvider>,
    );
    const nav = screen.getByRole("navigation", { name: "Main" });
    const orbit = await within(nav).findByRole("button", { name: /orbit/ });
    expect(orbit.querySelector("[data-color]")?.getAttribute("data-color")).toBe(colorFor("orbit"));
  });
});
