import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type FakeDocument = {
  visibilityState: DocumentVisibilityState;
  listeners: (() => void)[];
  addEventListener: (type: string, listener: () => void) => void;
};

let fakeDocument: FakeDocument;
let sendBeacon: ReturnType<typeof vi.fn>;

function hidePage(): void {
  fakeDocument.visibilityState = "hidden";
  fakeDocument.listeners.forEach((listener) => {
    listener();
  });
}

function sentEvents(call: number) {
  return JSON.parse(sendBeacon.mock.calls[call][1].parts[0]).events;
}

async function loadLogger() {
  const { Logger } = await import("../../src/logger");
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
  vi.stubGlobal("location", { pathname: "/home", href: "https://app.example/home?a=1" });
  vi.stubGlobal("navigator", { sendBeacon });
  vi.stubGlobal(
    "Blob",
    class {
      constructor(
        public parts: string[],
        public options: { type: string },
      ) {}
    },
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("Logger", () => {
  it("sends the queued events as one batch on each 1 minute interval", async () => {
    const Logger = await loadLogger();
    Logger.init([]);
    Logger.info("started");
    Logger.error("boom", new Error("it broke"), { code: 42 });
    await vi.advanceTimersByTimeAsync(59_999);
    expect(sendBeacon).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(sendBeacon).toHaveBeenCalledTimes(1);
    expect(sendBeacon.mock.calls[0][0]).toBe("/log");
    expect(sendBeacon.mock.calls[0][1].options).toEqual({ type: "application/json" });
    expect(sentEvents(0)).toEqual([
      {
        ts: "2026-10-25T20:00:00.000+08:00",
        action: "started",
        latencyMs: null,
        level: "info",
        errorMessage: null,
        context: {},
        extra: {},
      },
      expect.objectContaining({
        level: "error",
        errorMessage: "it broke",
        action: "boom",
        context: {},
        extra: { code: 42 },
      }),
    ]);

    await vi.advanceTimersByTimeAsync(60_000);
    expect(sendBeacon).toHaveBeenCalledTimes(1);

    Logger.info("later");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(sendBeacon).toHaveBeenCalledTimes(2);
    expect(sentEvents(1)).toHaveLength(1);
  });

  it("sends the queue when the page is hidden", async () => {
    const Logger = await loadLogger();
    Logger.init([]);
    Logger.warn("leaving");
    hidePage();
    expect(sendBeacon).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(60_000);
    expect(sendBeacon).toHaveBeenCalledTimes(1);
  });

  it("masks the value of a matching extra key and each match in a string value", async () => {
    const Logger = await loadLogger();
    Logger.init([/^(password|token)$/i, /secret=\S+/]);
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
    Logger.init([]);
    Logger.info("login", { password: "hunter2" });
    hidePage();
    expect(sentEvents(0)[0].extra).toEqual({ password: "hunter2" });
  });

  it("sends the events again after a refused beacon, before the newer events", async () => {
    const Logger = await loadLogger();
    Logger.init([]);
    sendBeacon.mockReturnValueOnce(false);
    Logger.info("first");
    await vi.advanceTimersByTimeAsync(60_000);
    Logger.info("second");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(sendBeacon).toHaveBeenCalledTimes(2);
    expect(sentEvents(1).map((event: { action: string }) => event.action)).toEqual(["first", "second"]);
  });

  it("sends the queue on pagehide", async () => {
    const windowListeners: Record<string, () => void> = {};
    vi.stubGlobal("window", {
      addEventListener(type: string, listener: () => void) {
        windowListeners[type] = listener;
      },
    });
    const Logger = await loadLogger();
    Logger.init([]);
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
    Logger.init([]);
    windowListeners.error({ message: "boom" });
    windowListeners.error({ message: "" });
    windowListeners.unhandledrejection({ reason: new Error("rejected") });
    windowListeners.unhandledrejection({ reason: 42 });
    hidePage();
    const sent = sentEvents(0).map(
      (event: { level: string; action: string; errorMessage: string }) => [
        event.level,
        event.action,
        event.errorMessage,
      ],
    );
    expect(sent).toEqual([
      ["error", "WINDOW/error", "boom"],
      ["error", "WINDOW/error", "window error"],
      ["error", "WINDOW/unhandledrejection", "rejected"],
      ["error", "WINDOW/unhandledrejection", "unhandled rejection"],
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
    Logger.init([]);
    windowListeners.error({ message: "", target: new FakeElement() });
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
    Logger.init([/secret-\d+/]);
    windowListeners.error({ message: `bad secret-42 ${"x".repeat(5000)}` });
    hidePage();
    const [event] = sentEvents(0);
    expect(event.action).toBe("WINDOW/error");
    expect(event.errorMessage).toContain("bad *** ");
    expect(event.errorMessage).not.toContain("secret-42");
  });

  it("records warn with the error text and masks the call extra", async () => {
    const Logger = await loadLogger();
    Logger.init([/^token$/]);
    Logger.warn("slow", { token: "abc", step: 1 }, new Error("timeout"));
    Logger.warn("odd", {}, new Error("plain"));
    Logger.warn("none");
    hidePage();
    const [first, second, third] = sentEvents(0);
    expect(first).toMatchObject({ level: "warn", errorMessage: "timeout" });
    expect(first.extra).toEqual({ token: "***", step: 1 });
    expect(second.errorMessage).toBe("plain");
    expect(third.errorMessage).toBeNull();
  });

  it("sends a fallback error message when the message is empty", async () => {
    const Logger = await loadLogger();
    Logger.init([]);
    Logger.error("a", new Error(""));
    Logger.error("b", new Error("   "));
    Logger.error("c", new Error(""));
    hidePage();
    const events = sentEvents(0);
    expect(events.map((event: { errorMessage: string }) => event.errorMessage)).toEqual([
      "unknown error",
      "unknown error",
      "unknown error",
    ]);
  });

  describe("capture", () => {
    it("records an info event with the latency and returns the output", async () => {
      const Logger = await loadLogger();
      Logger.init([]);
      const pending = Logger.capture("cart/count", async (extra) => {
        await new Promise((resolve) => setTimeout(resolve, 25));
        extra.count = 7;
        return 7;
      });
      await vi.advanceTimersByTimeAsync(25);

      expect(await pending).toBe(7);
      hidePage();
      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0]).toMatchObject({ action: "cart/count", level: "info", latencyMs: 25, errorMessage: null });
      expect(events[0].extra).toEqual({ count: 7 });
    });

    it("records an error event and rethrows when the work rejects", async () => {
      const Logger = await loadLogger();
      Logger.init([]);
      await expect(Logger.capture("save", () => Promise.reject(new Error("denied")))).rejects.toThrow("denied");
      await expect(Logger.capture("save", () => Promise.reject(new Error("")))).rejects.toThrow();
      hidePage();
      const events = sentEvents(0);
      expect(events.map((event: { level: string; errorMessage: string }) => [event.level, event.errorMessage])).toEqual([
        ["error", "denied"],
        ["error", "unknown error"],
      ]);
    });
  });

  describe("context", () => {
    it("puts the added context in each later event", async () => {
      const Logger = await loadLogger();
      Logger.init([]);
      Logger.info("before");
      Logger.addContext("user", "u1");
      Logger.addContext("beta", "on");
      Logger.addContext("user", "u2");
      Logger.info("after");
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events[0].context).toEqual({});
      expect(events[1].context).toEqual({ user: "u2", beta: "on" });
    });

    it("removes the named keys", async () => {
      const Logger = await loadLogger();
      Logger.init([]);
      Logger.addContext("user", "u1");
      Logger.addContext("beta", "on");
      Logger.removeContext("user", "missing");
      Logger.info("after");
      vi.advanceTimersByTime(60_000);

      expect(sentEvents(0)[0].context).toEqual({ beta: "on" });
    });
  });
});
