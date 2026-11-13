import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let listeners: Record<string, (event: any) => void>;

async function setup(handler: (error: unknown) => void | Promise<void>) {
  const { setupExceptionHandler } = await import("../src/exception-handler");
  setupExceptionHandler(handler);
  // The classes must come from the same module copy as the handler, after vi.resetModules
  return import("../src/ajax");
}

beforeEach(() => {
  vi.resetModules();
  listeners = {};
  vi.stubGlobal("window", {
    addEventListener(type: string, listener: (event: any) => void) {
      listeners[type] = listener;
    },
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("setupExceptionHandler", () => {
  it("gives only ApiError and NetworkConnectionException to the handler", async () => {
    const handler = vi.fn();
    const { ApiError, NetworkConnectionException } = await setup(handler);
    const apiError = new ApiError(400, { message: "bad" });
    const networkError = new NetworkConnectionException("down", "/ajax/item");

    const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

    listeners.error({ error: apiError });
    await flush();
    listeners.unhandledrejection({ reason: networkError });
    await flush();
    listeners.error({ error: new Error("other") });
    listeners.unhandledrejection({ reason: "text" });

    expect(handler.mock.calls).toEqual([[apiError], [networkError]]);
  });

  it("ignores an error while the handler is running", async () => {
    let finish = () => {};
    const handler = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    const { ApiError } = await setup(handler);

    listeners.error({ error: new ApiError(400, null) });
    listeners.error({ error: new ApiError(500, null) });
    expect(handler).toHaveBeenCalledOnce();

    finish();
    await Promise.resolve();
    await Promise.resolve();
    listeners.error({ error: new ApiError(500, null) });
    expect(handler).toHaveBeenCalledTimes(2);
  });

  it("throws when it is called twice", async () => {
    await setup(vi.fn());
    await expect(setup(vi.fn())).rejects.toThrow("called twice");
  });
});
