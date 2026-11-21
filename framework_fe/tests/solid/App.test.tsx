import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { JSX } from "@solidjs/web";

function Boom(): never {
    throw new Error("boom");
}

async function sentEvents() {
    const body = sendBeacon.mock.calls.at(-1)?.[1] as Blob;
    const text = await body.text();
    return JSON.parse(text).events as { action: string; level: string; errorMessage: string | null }[];
}

async function load() {
    const [{ App }, { Logger }] = await Promise.all([import("../../src/solid/App"), import("../../src/logger")]);
    Logger.init([]);

    class TestApp extends App<{ count: number }> {
        started = 0;
        fail = false;

        constructor() {
            super({ count: 0 }, (reset) => (
                <button type="button" onClick={reset}>
                    fallback text
                </button>
            ));
        }

        protected onStart = async () => {
            this.started += 1;
            if (this.fail) throw new Error("start failed");
            this.setState((state) => {
                state.count = 1;
            });
        };

        mount(root: HTMLElement, children: () => JSX.Element) {
            return this.start(root, children);
        }
    }

    return new TestApp();
}

let sendBeacon: ReturnType<typeof vi.fn>;
let dispose: (() => void) | undefined;

beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-25T12:00:00Z"));
    sendBeacon = vi.fn().mockReturnValue(true);
    vi.stubGlobal("navigator", { sendBeacon });
});

afterEach(() => {
    dispose?.();
    dispose = undefined;
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

describe("App", () => {
    it("renders the children, calls onStart one time, and logs the start", async () => {
        const app = await load();
        const root = document.createElement("div");
        dispose = app.mount(root, () => <p>count {app.store().count}</p>);

        expect(root.textContent).toContain("count 0");
        expect(app.started).toBe(1);

        await vi.advanceTimersByTimeAsync(60_000);
        expect(root.textContent).toContain("count 1");
        const events = await sentEvents();
        expect(events.find((event) => event.action === "APP/start")?.level).toBe("info");
    });

    it("shows the error fallback and logs the error when a child throws", async () => {
        const app = await load();
        const root = document.createElement("div");
        dispose = app.mount(root, () => <Boom />);

        expect(root.textContent).toContain("fallback text");

        await vi.advanceTimersByTimeAsync(60_000);
        const events = await sentEvents();
        expect(events.find((event) => event.action === "ERROR-BOUNDARY/caughtError")?.errorMessage).toContain("boom");
    });

    it("logs the error and keeps the page when onStart fails", async () => {
        const app = await load();
        app.fail = true;
        const root = document.createElement("div");
        dispose = app.mount(root, () => <p>page text</p>);

        await vi.advanceTimersByTimeAsync(60_000);
        expect(root.textContent).toContain("page text");
        const events = await sentEvents();
        const logged = events.find((event) => event.action === "APP/start");
        expect(logged?.level).toBe("error");
        expect(logged?.errorMessage).toContain("start failed");
    });
});
