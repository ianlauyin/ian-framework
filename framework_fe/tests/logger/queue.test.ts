import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";
import {Queue} from "../../src/logger/queue";
import type {ClientLogEvent} from "../../src/logger/types";

let sendBeacon: ReturnType<typeof vi.fn>;

function event(action: string, note = ""): ClientLogEvent {
    return {
        ts: "2026-10-25T20:00:00.000+08:00",
        action,
        latencyMs: null,
        level: "info",
        errorMessage: null,
        errorName: null,
        stack: null,
        context: {},
        extra: {note},
    };
}

function sentActions(): string[][] {
    return sendBeacon.mock.calls.map(call => JSON.parse(call[1].parts[0]).events.map((sent: ClientLogEvent) => sent.action));
}

beforeEach(() => {
    sendBeacon = vi.fn().mockReturnValue(true);
    vi.stubGlobal("navigator", {sendBeacon});
    vi.stubGlobal(
        "Blob",
        class {
            constructor(
                public parts: string[],
                public options: {type: string}
            ) {}
        }
    );
});

afterEach(() => {
    vi.unstubAllGlobals();
});

describe("Queue", () => {
    it("sends nothing when it is empty and sends the events one time", () => {
        const queue = new Queue();
        queue.send();
        expect(sendBeacon).not.toHaveBeenCalled();
        queue.push(event("a"));
        queue.push(event("b"));
        queue.send();
        queue.send();
        expect(sendBeacon).toHaveBeenCalledTimes(1);
        expect(sendBeacon.mock.calls[0][0]).toBe("/log");
        expect(sentActions()).toEqual([["a", "b"]]);
    });

    it("keeps the events after a refused beacon and sends them before the newer events", () => {
        const queue = new Queue();
        sendBeacon.mockReturnValueOnce(false);
        queue.push(event("a"));
        queue.send();
        queue.push(event("b"));
        queue.send();
        expect(sentActions()).toEqual([["a"], ["a", "b"]]);
    });

    it("splits a large queue and drops an event that is larger than one beacon", () => {
        const queue = new Queue();
        queue.push(event("a", "x".repeat(40_000)));
        queue.push(event("too-large", "x".repeat(70_000)));
        queue.push(event("b", "x".repeat(40_000)));
        queue.push(event("c"));
        queue.send();
        expect(sentActions()).toEqual([["a"], ["b", "c"]]);
    });

    it("drops the oldest event when it holds more than 1,000 events", () => {
        const queue = new Queue();
        for (let index = 0; index <= 1000; index += 1) queue.push(event(String(index)));
        queue.send();
        const actions = sentActions().flat();
        expect(actions).toHaveLength(1000);
        expect(actions[0]).toBe("1");
    });
});
