import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { PRIORITY_COLOR } from "../lib/colors";
import { PriorityChip, PriorityIcon } from "./PriorityIcon";

describe("priority", () => {
  it.each([
    ["urgent", "Urgent"],
    ["high", "High"],
    ["medium", "Medium"],
    ["low", "Low"],
  ] as const)("chip shows label and colour for %s", (priority, label) => {
    const { container } = render(<PriorityChip priority={priority} />);
    expect(screen.getByText(label)).toBeInTheDocument();
    const lit = container.querySelector(priority === "urgent" ? "rect" : "[data-bar='on']");
    expect(lit?.getAttribute("fill")).toBe(PRIORITY_COLOR[priority]);
  });

  it("no chip for none", () => {
    const { container } = render(<PriorityChip priority="none" />);
    expect(container).toBeEmptyDOMElement();
  });

  it("list icon uses the priority colour", () => {
    const { container } = render(<PriorityIcon priority="medium" />);
    const bars = [...container.querySelectorAll("[data-bar='on']")].map((b) => b.getAttribute("fill"));
    expect(bars).toEqual([PRIORITY_COLOR.medium, PRIORITY_COLOR.medium]);
  });
});
