import { describe, expect, it } from "vitest";

import { Mask } from "../../src/logger/mask";

describe("Mask", () => {
  it("masks every match when the pattern has no g flag", () => {
    const mask = new Mask([/secret/i]);

    expect(mask.text("secret and SECRET")).toBe("*** and ***");
  });
});
