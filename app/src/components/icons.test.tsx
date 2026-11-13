import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusIcon } from "./StatusIcon";
import { PriorityIcon } from "./PriorityIcon";
import type { Priority, Status } from "../lib/types";

const statuses: [Status, string][] = [
  ["backlog", "Backlog"],
  ["todo", "Todo"],
  ["in_progress", "In progress"],
  ["done", "Done"],
  ["canceled", "Canceled"],
];

const priorities: [Priority, string][] = [
  ["none", "No priority"],
  ["low", "Low priority"],
  ["medium", "Medium priority"],
  ["high", "High priority"],
  ["urgent", "Urgent"],
];

describe("StatusIcon", () => {
  it.each(statuses)("labels %s", (status, label) => {
    render(<StatusIcon status={status} />);
    expect(screen.getByRole("img", { name: label })).toBeInTheDocument();
  });

  it("half fills in progress", () => {
    const { container } = render(<StatusIcon status="in_progress" />);
    expect(container.querySelector('[data-fill="half"]')).not.toBeNull();
  });
});

describe("PriorityIcon", () => {
  it.each(priorities)("labels %s", (priority, label) => {
    render(<PriorityIcon priority={priority} />);
    expect(screen.getByRole("img", { name: label })).toBeInTheDocument();
  });

  it("draws as many bars as the level", () => {
    const { container } = render(<PriorityIcon priority="medium" />);
    expect(container.querySelectorAll('[data-bar="on"]').length).toBe(2);
  });
});
