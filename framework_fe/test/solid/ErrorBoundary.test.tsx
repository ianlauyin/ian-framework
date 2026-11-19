// @vitest-environment happy-dom

import { render } from "@solidjs/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function Boom(): never {
  throw new Error("boom");
}

async function sentEvents() {
  const body = sendBeacon.mock.calls.at(-1)?.[1] as Blob;
  const text = await body.text();
  return JSON.parse(text).events as { action: string; errorMessage: string | null }[];
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

describe("ErrorBoundary", () => {
  it("shows the fallback and logs the error", async () => {
    const [{ ErrorBoundary }, { Logger }] = await Promise.all([
      import("../../src/solid/ErrorBoundary"),
      import("../../src/logger"),
    ]);
    Logger.init([]);
    const root = document.createElement("div");
    let args: unknown[] = [];
    dispose = render(
      () => (
        <ErrorBoundary
          fallback={(...received) => {
            args = received;
            return <p>fallback text</p>;
          }}
        >
          <Boom />
        </ErrorBoundary>
      ),
      root,
    );

    expect(root.textContent).toContain("fallback text");
    expect(args).toHaveLength(1);
    expect(typeof args[0]).toBe("function");

    await vi.advanceTimersByTimeAsync(60_000);
    const events = await sentEvents();
    const logged = events.find((event) => event.action === "ERROR-BOUNDARY/caughtError");
    expect(logged?.errorMessage).toContain("boom");
  });
});
