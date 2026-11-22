import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";
import {render} from "@solidjs/web";

function Boom(): never {
    throw new Error("boom");
}

async function sentEvents() {
    const body = sendBeacon.mock.calls.at(-1)?.[1] as Blob;
    const text = await body.text();
    return JSON.parse(text).events as {action: string; errorMessage: string | null}[];
}

let sendBeacon: ReturnType<typeof vi.fn>;
let dispose: (() => void) | undefined;

beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-25T12:00:00Z"));
    sendBeacon = vi.fn().mockReturnValue(true);
    vi.stubGlobal("navigator", {sendBeacon});
});

afterEach(() => {
    dispose?.();
    dispose = undefined;
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

describe("ErrorBoundary", () => {
    it("shows the fallback and logs the error", async () => {
        const [{ErrorBoundary}, {Logger}] = await Promise.all([import("../../src/solid/ErrorBoundary"), import("../../src/logger")]);
        Logger.init({maskPatterns: []});
        const root = document.createElement("div");
        let props: {reset: () => void} | undefined;
        dispose = render(
            () => (
                <ErrorBoundary
                    fallback={received => {
                        props = received;
                        return <p>fallback text</p>;
                    }}
                >
                    <Boom />
                </ErrorBoundary>
            ),
            root
        );

        expect(root.textContent).toContain("fallback text");
        expect(typeof props?.reset).toBe("function");

        await vi.advanceTimersByTimeAsync(60_000);
        const events = await sentEvents();
        const logged = events.find(event => event.action === "ERROR-BOUNDARY/caughtError");
        expect(logged?.errorMessage).toContain("boom");
    });

    it("shows the children again after reset", async () => {
        const {ErrorBoundary} = await import("../../src/solid/ErrorBoundary");
        const root = document.createElement("div");
        let broken = true;
        let reset = () => {};
        const Content = () => {
            if (broken) throw new Error("boom");
            return <p>content text</p>;
        };
        dispose = render(
            () => (
                <ErrorBoundary
                    fallback={props => {
                        reset = props.reset;
                        return <p>fallback text</p>;
                    }}
                >
                    <Content />
                </ErrorBoundary>
            ),
            root
        );
        expect(root.textContent).toContain("fallback text");

        broken = false;
        reset();
        await vi.advanceTimersByTimeAsync(0);
        expect(root.textContent).toContain("content text");
        expect(root.textContent).not.toContain("fallback text");
    });
});
