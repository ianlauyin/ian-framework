import { describe, expect, it } from "vitest";

import { hktIsoString } from "../src/time";

describe("hktIsoString", () => {
  it("gives the HKT time with the +08:00 offset", () => {
    expect(hktIsoString(new Date("2026-10-27T06:30:00.123Z"))).toBe("2026-10-27T14:30:00.123+08:00");
    expect(hktIsoString(new Date("2026-12-31T20:00:00.000Z"))).toBe("2027-01-01T04:00:00.000+08:00");
  });
});
