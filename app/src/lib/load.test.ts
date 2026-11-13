import { describe, expect, it } from "vitest";
import { loadPercent } from "./load";

describe("loadPercent", () => {
  it("is the load over the cores", () => {
    expect(loadPercent(0.74, 2)).toBe("37%");
    expect(loadPercent(3, 2)).toBe("150%");
    expect(loadPercent(0.5, 0)).toBeNull();
  });
});
