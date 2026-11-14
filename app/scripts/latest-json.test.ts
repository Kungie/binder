import { describe, expect, it } from "vitest";
import { latestJson } from "./latest-json";

describe("latestJson", () => {
  it("describes the release for the updater", () => {
    expect(latestJson("2.4.0", "Fixes", "c2ln", new Date("2026-11-14T12:00:00Z"))).toEqual({
      version: "2.4.0",
      notes: "Fixes",
      pub_date: "2026-11-14T12:00:00.000Z",
      platforms: {
        "darwin-aarch64": { signature: "c2ln", url: "https://github.com/Kungie/binder/releases/download/v2.4.0/Binder.app.tar.gz" },
      },
    });
  });
});
