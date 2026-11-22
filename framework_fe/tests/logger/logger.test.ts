import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";

interface FakeDocument {
    visibilityState: DocumentVisibilityState;
    listeners: (() => void)[];
    addEventListener: (type: string, listener: () => void) => void;
}

let fakeDocument: FakeDocument;
let sendBeacon: ReturnType<typeof vi.fn>;

function hidePage(): void {
    fakeDocument.visibilityState = "hidden";
    fakeDocument.listeners.forEach(listener => {
        listener();
    });
}

function sentEvents(call: number) {
    return JSON.parse(sendBeacon.mock.calls[call][1].parts[0]).events;
}

async function loadLogger() {
    const {Logger} = await import("../../src/logger/logger");
    return Logger;
}

beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-25T12:00:00Z"));
    fakeDocument = {
        visibilityState: "visible",
        listeners: [],
        addEventListener(type, listener) {
            if (type === "visibilitychange") fakeDocument.listeners.push(listener);
        },
    };
    sendBeacon = vi.fn().mockReturnValue(true);
    vi.stubGlobal("document", fakeDocument);
    vi.stubGlobal("window", {addEventListener() {}});
    vi.stubGlobal("location", {pathname: "/home", href: "https://app.example/home?a=1"});
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
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

describe("Logger", () => {
    it("sends the queued events as one batch on each 1 minute interval", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.info("started");
        Logger.error("boom", new Error("it broke"), {code: 42});
        await vi.advanceTimersByTimeAsync(59_999);
        expect(sendBeacon).not.toHaveBeenCalled();

        await vi.advanceTimersByTimeAsync(1);
        expect(sendBeacon).toHaveBeenCalledTimes(1);
        expect(sendBeacon.mock.calls[0][0]).toBe("/log");
        expect(sendBeacon.mock.calls[0][1].options).toEqual({type: "application/json"});
        expect(sentEvents(0)).toEqual([
            {
                ts: "2026-10-25T20:00:00.000+08:00",
                action: "started",
                latencyMs: null,
                level: "info",
                errorMessage: null,
                errorName: null,
                stack: null,
                context: {},
                extra: {},
            },
            expect.objectContaining({
                level: "error",
                errorMessage: "it broke",
                action: "boom",
                context: {},
                extra: {code: 42},
            }),
        ]);

        await vi.advanceTimersByTimeAsync(60_000);
        expect(sendBeacon).toHaveBeenCalledTimes(1);

        Logger.info("later");
        await vi.advanceTimersByTimeAsync(60_000);
        expect(sendBeacon).toHaveBeenCalledTimes(2);
        expect(sentEvents(1)).toHaveLength(1);
    });

    it("sends on the interval that the options give", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: [], sendInterval: 5000});
        Logger.info("started");
        await vi.advanceTimersByTimeAsync(4999);
        expect(sendBeacon).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(1);
        expect(sendBeacon).toHaveBeenCalledTimes(1);
    });

    it("throws and does not start when the interval is less than 1 second", async () => {
        const Logger = await loadLogger();
        for (const sendInterval of [999, 0, Number.NaN]) {
            expect(() => Logger.init({maskPatterns: [], sendInterval})).toThrow("sendInterval must be 1000 or more");
        }
        Logger.init({maskPatterns: [], sendInterval: 1000});
    });

    it("sends the queue when the page is hidden", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.warn("leaving");
        hidePage();
        expect(sendBeacon).toHaveBeenCalledTimes(1);

        await vi.advanceTimersByTimeAsync(60_000);
        expect(sendBeacon).toHaveBeenCalledTimes(1);
    });

    it("masks the value of a matching extra key and each match in a string value", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: [/^(password|token)$/i, /secret=\S+/]});
        Logger.info("login secret=keep", {
            password: "hunter2",
            TOKEN: 1234,
            note: "a secret=x and secret=y",
            code: 7,
        });
        hidePage();
        const [event] = sentEvents(0);
        expect(event.action).toBe("login secret=keep");
        expect(event.extra).toEqual({
            password: "***",
            TOKEN: "***",
            note: "a *** and ***",
            code: 7,
        });
    });

    it("keeps the extra values when no mask pattern is set", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.info("login", {password: "hunter2"});
        hidePage();
        expect(sentEvents(0)[0].extra).toEqual({password: "hunter2"});
    });

    it("keeps the extra of an event when the caller changes the object later", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        const extra = {step: 1};
        Logger.info("login", extra);
        extra.step = 2;
        hidePage();
        expect(sentEvents(0)[0].extra).toEqual({step: 1});
    });

    it("sends the events again after a refused beacon, before the newer events", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        sendBeacon.mockReturnValueOnce(false);
        Logger.info("first");
        await vi.advanceTimersByTimeAsync(60_000);
        Logger.info("second");
        await vi.advanceTimersByTimeAsync(60_000);
        expect(sendBeacon).toHaveBeenCalledTimes(2);
        expect(sentEvents(1).map((event: {action: string}) => event.action)).toEqual(["first", "second"]);
    });

    it("sends a large queue in batches that are each below the beacon limit", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        for (let index = 0; index < 30; index += 1) Logger.info(`event-${index}`, {text: "é".repeat(2000)});
        hidePage();

        expect(sendBeacon.mock.calls.length).toBeGreaterThan(1);
        const actions: string[] = [];
        sendBeacon.mock.calls.forEach((call, index) => {
            expect(new TextEncoder().encode(call[1].parts[0]).length).toBeLessThanOrEqual(60_000);
            actions.push(...sentEvents(index).map((event: {action: string}) => event.action));
        });
        expect(actions).toEqual(Array.from({length: 30}, (_, index) => `event-${index}`));
    });

    it("drops an event that is larger than the beacon limit and sends the other events", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.info("large", {text: "x".repeat(70_000)});
        Logger.info("small");
        hidePage();

        expect(sendBeacon).toHaveBeenCalledOnce();
        expect(sentEvents(0).map((event: {action: string}) => event.action)).toEqual(["small"]);
    });

    it("drops the oldest events when the queue is full", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        sendBeacon.mockReturnValue(false);
        for (let index = 0; index < 1001; index += 1) Logger.info(`event-${index}`);
        hidePage();
        sendBeacon.mockReturnValue(true);
        hidePage();

        const actions = sendBeacon.mock.calls.slice(1).flatMap((_, index) => sentEvents(index + 1).map((event: {action: string}) => event.action));
        expect(actions).toHaveLength(1000);
        expect(actions[0]).toBe("event-1");
        expect(actions.at(-1)).toBe("event-1000");
    });

    it("throws when init is called twice", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        expect(() => Logger.init({maskPatterns: []})).toThrow("called twice");
    });

    it("sends the queue on pagehide", async () => {
        const windowListeners: Record<string, () => void> = {};
        vi.stubGlobal("window", {
            addEventListener(type: string, listener: () => void) {
                windowListeners[type] = listener;
            },
        });
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.info("started");
        windowListeners.pagehide();
        expect(sendBeacon).toHaveBeenCalledOnce();
        expect(sentEvents(0)).toHaveLength(1);
        windowListeners.pagehide();
        expect(sendBeacon).toHaveBeenCalledOnce();
    });

    it("records a window error and an unhandled rejection as error events", async () => {
        const windowListeners: Record<string, (event: unknown) => void> = {};
        vi.stubGlobal("window", {
            addEventListener(type: string, listener: (event: unknown) => void) {
                windowListeners[type] = listener;
            },
        });
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        windowListeners.error({message: "boom"});
        windowListeners.error({message: ""});
        windowListeners.unhandledrejection({reason: new Error("rejected")});
        windowListeners.unhandledrejection({reason: 42});
        hidePage();
        const sent = sentEvents(0).map((event: {level: string; action: string; errorMessage: string}) => [event.level, event.action, event.errorMessage]);
        expect(sent).toEqual([
            ["error", "WINDOW/error", "boom"],
            ["error", "WINDOW/error", "window error"],
            ["error", "WINDOW/unhandledrejection", "rejected"],
            ["error", "WINDOW/unhandledrejection", "42"],
        ]);
    });

    it("records a failed resource load with the tag and the source", async () => {
        const windowListeners: Record<string, (event: unknown) => void> = {};
        class FakeElement {
            tagName = "SCRIPT";
            getAttribute(name: string) {
                return name === "src" ? "/assets/app.js" : null;
            }
        }
        vi.stubGlobal("Element", FakeElement);
        vi.stubGlobal("window", {
            addEventListener(type: string, listener: (event: unknown) => void) {
                windowListeners[type] = listener;
            },
        });
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        windowListeners.error({message: "", target: new FakeElement()});
        hidePage();
        expect(sentEvents(0)[0].errorMessage).toBe("DOM source error: <script> /assets/app.js");
    });

    it("keeps a long window error message out of the action and masks it", async () => {
        const windowListeners: Record<string, (event: unknown) => void> = {};
        vi.stubGlobal("window", {
            addEventListener(type: string, listener: (event: unknown) => void) {
                windowListeners[type] = listener;
            },
        });
        const Logger = await loadLogger();
        Logger.init({maskPatterns: [/secret-\d+/]});
        windowListeners.error({message: `bad secret-42 ${"x".repeat(5000)}`});
        hidePage();
        const [event] = sentEvents(0);
        expect(event.action).toBe("WINDOW/error");
        expect(event.errorMessage).toContain("bad *** ");
        expect(event.errorMessage).not.toContain("secret-42");
    });

    it("records warn with the error text and masks the call extra", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: [/^token$/]});
        Logger.warn("slow", new Error("timeout"), {token: "abc", step: 1});
        Logger.warn("odd", new Error("plain"));
        Logger.warn("none");
        hidePage();
        const [first, second, third] = sentEvents(0);
        expect(first).toMatchObject({level: "warn", errorMessage: "timeout"});
        expect(first.extra).toEqual({token: "***", step: 1});
        expect(second.errorMessage).toBe("plain");
        expect(third.errorMessage).toBeNull();
    });

    it("sends a fallback error message when the message is empty", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        Logger.error("a", new Error(""));
        Logger.error("b", new Error("   "));
        Logger.error("c", new Error(""));
        hidePage();
        const events = sentEvents(0);
        expect(events.map((event: {errorMessage: string}) => event.errorMessage)).toEqual(["unknown error", "unknown error", "unknown error"]);
    });

    describe("trackNavigation", () => {
        let popstate: (() => void) | undefined;
        let location: {href: string};

        function go(url: string): void {
            location.href = `https://app.example${url}`;
        }

        beforeEach(() => {
            popstate = undefined;
            location = {href: "https://app.example/home?a=1"};
            vi.stubGlobal("window", {
                location,
                history: {
                    pushState: (_data: unknown, _unused: string, url: string) => go(url),
                    replaceState: (_data: unknown, _unused: string, url: string) => go(url),
                },
                addEventListener(type: string, listener: () => void) {
                    if (type === "popstate") popstate = listener;
                },
            });
        });

        async function requestUrl(Logger: Awaited<ReturnType<typeof loadLogger>>, call: number): Promise<string> {
            Logger.info("later");
            hidePage();
            return sentEvents(call)[0].context.requestUrl;
        }

        it("stores the page URL and records no event", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            Logger.trackNavigation();
            window.history.pushState({}, "", "/next");
            Logger.info("later");
            hidePage();

            expect(sentEvents(0).map((event: {action: string}) => event.action)).toEqual(["later"]);
        });

        it("stores the new page URL after pushState, replaceState and popstate", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            Logger.trackNavigation();
            expect(await requestUrl(Logger, 0)).toBe("https://app.example/home?a=1");
            window.history.pushState({}, "", "/next");
            expect(await requestUrl(Logger, 1)).toBe("https://app.example/next");
            window.history.replaceState({}, "", "/other");
            expect(await requestUrl(Logger, 2)).toBe("https://app.example/other");
            go("/home");
            popstate?.();
            expect(await requestUrl(Logger, 3)).toBe("https://app.example/home");
        });

        it("masks the value of a secret parameter in the page URL", async () => {
            go("/home?token=abc");
            const Logger = await loadLogger();
            Logger.init({maskPatterns: [/token/i]});
            Logger.trackNavigation();

            expect(await requestUrl(Logger, 0)).toBe("https://app.example/home?token=***");
        });

        it("wraps history one time when it is called two times", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            Logger.trackNavigation();
            const pushState = window.history.pushState;
            Logger.trackNavigation();

            expect(window.history.pushState).toBe(pushState);
        });
    });

    describe("capture", () => {
        it("records an info event with the latency and returns the output", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            const pending = Logger.capture("cart/count", async extra => {
                await new Promise(resolve => setTimeout(resolve, 25));
                extra.count = 7;
                return 7;
            });
            await vi.advanceTimersByTimeAsync(25);

            expect(await pending).toBe(7);
            hidePage();
            const events = sentEvents(0);
            expect(events).toHaveLength(1);
            expect(events[0]).toMatchObject({action: "cart/count", level: "info", latencyMs: 25, errorMessage: null});
            expect(events[0].extra).toEqual({count: 7});
        });

        it("records an error event and rethrows when the work rejects", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            await expect(Logger.capture("save", () => Promise.reject(new Error("denied")))).rejects.toThrow("denied");
            await expect(Logger.capture("save", () => Promise.reject(new Error("")))).rejects.toThrow();
            await expect(Logger.capture("save", () => Promise.reject("text reason"))).rejects.toBe("text reason");
            hidePage();
            const events = sentEvents(0);
            expect(events.map((event: {level: string; errorMessage: string}) => [event.level, event.errorMessage])).toEqual([
                ["error", "denied"],
                ["error", "unknown error"],
                ["error", "text reason"],
            ]);
        });
    });

    describe("context", () => {
        it("puts the added context in each later event", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            Logger.info("before");
            Logger.addContext("user", "u1");
            Logger.addContext("beta", "on");
            Logger.addContext("user", "u2");
            Logger.info("after");
            vi.advanceTimersByTime(60_000);

            const events = sentEvents(0);
            expect(events[0].context).toEqual({});
            expect(events[1].context).toEqual({user: "u2", beta: "on"});
        });

        it("removes the named keys", async () => {
            const Logger = await loadLogger();
            Logger.init({maskPatterns: []});
            Logger.addContext("user", "u1");
            Logger.addContext("beta", "on");
            Logger.removeContext("user", "missing");
            Logger.info("after");
            vi.advanceTimersByTime(60_000);

            expect(sentEvents(0)[0].context).toEqual({beta: "on"});
        });
    });

    it("sends the name and the masked stack of an error, without the header line", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: [/secret-\w+/]});
        const error = new TypeError("it broke");
        error.stack = "TypeError: it broke\n    at load (app.js:1:2)\n    at secret-abc (app.js:3:4)";
        Logger.error("typed", error);
        const plain = new Error("plain");
        plain.stack = `at only (app.js:5:6)${"x".repeat(5000)}`;
        Logger.warn("plain", plain);
        hidePage();
        const [typed, warned] = sentEvents(0);
        expect(typed.errorName).toBe("TypeError");
        expect(typed.stack).toBe("    at load (app.js:1:2)\n    at *** (app.js:3:4)");
        expect(warned.errorName).toBeNull();
        expect(warned.stack).toHaveLength(4096);
        expect(warned.stack.startsWith("at only (app.js:5:6)")).toBe(true);
    });

    it("sends no stack for a thrown value that is not an Error, and no name or stack without an error", async () => {
        const Logger = await loadLogger();
        Logger.init({maskPatterns: []});
        await expect(
            Logger.capture("work", async () => {
                throw "text";
            })
        ).rejects.toBe("text");
        Logger.warn("no error");
        hidePage();
        const [thrown, warned] = sentEvents(0);
        expect(thrown.errorMessage).toBe("text");
        expect(thrown.errorName).toBeNull();
        expect(thrown.stack).toBeNull();
        expect(warned.errorName).toBeNull();
        expect(warned.stack).toBeNull();
    });
});
