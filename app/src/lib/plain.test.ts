import { describe, expect, it } from "vitest";
import { plainText } from "./plain";

describe("plainText", () => {
  it("drops Markdown marks and keeps the words", () => {
    expect(plainText("## Fixes\n- **Faster** [sync](https://x.y)\n* `ssh` errors")).toBe("Fixes\nFaster sync\nssh errors");
  });

  it("drops numbered list marks and underscores", () => {
    expect(plainText("1. _One_\n2. Two")).toBe("One\nTwo");
  });

  it("leaves plain text as it is", () => {
    expect(plainText("Updates from Settings.")).toBe("Updates from Settings.");
  });
});
