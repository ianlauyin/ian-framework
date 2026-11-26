import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";
import {buildEvent, errorWithoutStack, toError} from "../../src/logger/event";
import {Mask} from "../../src/logger/mask";

beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-25T12:00:00Z"));
});

afterEach(() => {
    vi.useRealTimers();
});

describe("buildEvent", () => {
    it("builds an event with no error fields and a copy of the context", () => {
        const context = {user: "u-1"};
        const event = buildEvent(new Mask([]), {action: "started", level: "info", context, extra: {page: 2}});
        context.user = "u-2";
        expect(event).toEqual({
            ts: "2026-10-25T12:00:00.000Z",
            action: "started",
            latencyMs: null,
            level: "info",
            errorMessage: null,
            errorName: null,
            stack: null,
            context: {user: "u-1"},
            extra: {page: 2},
        });
    });

    it("masks the message, the stack and the extra, and keeps the latency", () => {
        const error = new RangeError("bad secret-1");
        error.stack = "RangeError: bad secret-1\n    at secret-2 (app.js:1:2)";
        const event = buildEvent(new Mask([/secret-\d/]), {
            action: "work",
            level: "error",
            context: {},
            extra: {note: "secret-3"},
            error,
            latencyMs: 12,
        });
        expect(event.errorMessage).toBe("bad ***");
        expect(event.errorName).toBe("RangeError");
        expect(event.stack).toBe("    at *** (app.js:1:2)");
        expect(event.extra).toEqual({note: "***"});
        expect(event.latencyMs).toBe(12);
    });

    it("gives the fallback message only to an error event with an empty message", () => {
        const input = {action: "x", context: {}, extra: {}, error: errorWithoutStack(" ")};
        expect(buildEvent(new Mask([]), {...input, level: "error"}).errorMessage).toBe("unknown error");
        expect(buildEvent(new Mask([]), {...input, level: "warn"}).errorMessage).toBe(" ");
    });
});

describe("toError", () => {
    it("keeps an Error and makes an Error with no stack from a different value", () => {
        const error = new Error("boom");
        expect(toError(error)).toBe(error);
        const made = toError(42);
        expect(made.message).toBe("42");
        expect(made.stack).toBeUndefined();
    });
});
