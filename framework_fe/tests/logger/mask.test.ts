import {describe, expect, it} from "vitest";

import {Mask} from "../../src/logger/mask";

describe("Mask", () => {
    it("masks every match when the pattern has no g flag", () => {
        const mask = new Mask([/secret/i]);

        expect(mask.text("secret and SECRET")).toBe("*** and ***");
    });

    it("masks the value of a URL parameter when the key matches", () => {
        const mask = new Mask([/token/i, /secret/i]);

        expect(mask.url("https://app.example/home?a=1&token=abc&b=2#access_token=xyz&c=3")).toBe("https://app.example/home?a=1&token=***&b=2#access_token=***&c=3");
    });

    it("masks a match in the path and in a parameter value of a URL", () => {
        const mask = new Mask([/secret/i]);

        expect(mask.url("https://app.example/secret/home?a=secret")).toBe("https://app.example/***/home?a=***");
        expect(mask.url("https://app.example/secret")).toBe("https://app.example/***");
    });
});
