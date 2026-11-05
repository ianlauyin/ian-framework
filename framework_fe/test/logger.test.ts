import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type FakeDocument = {
  visibilityState: DocumentVisibilityState;
  listeners: (() => void)[];
  addEventListener: (type: string, listener: () => void) => void;
};

let fakeDocument: FakeDocument;
let fetchMock: ReturnType<typeof vi.fn>;

function hidePage(): void {
  fakeDocument.visibilityState = "hidden";
  fakeDocument.listeners.forEach((listener) => {
    listener();
  });
}

function sentEvents(call: number) {
  return JSON.parse(fetchMock.mock.calls[call][1].body).events;
}

async function loadLogger() {
  const { Logger } = await import("../src/logger");
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
  fetchMock = vi.fn().mockResolvedValue({ ok: true });
  vi.stubGlobal("document", fakeDocument);
  vi.stubGlobal("location", { pathname: "/home", href: "https://app.example/home?a=1" });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("Logger", () => {
  it("sends the queued events as one batch on each 1 minute interval", async () => {
    const Logger = await loadLogger();
    Logger.info("started");
    Logger.error("boom", "it broke", { code: 42, page: "/other" });
    await vi.advanceTimersByTimeAsync(59_999);
    expect(fetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][0]).toBe("/log");
    expect(fetchMock.mock.calls[0][1]).toMatchObject({
      method: "POST",
      credentials: "same-origin",
      keepalive: false,
      headers: { "Content-Type": "application/json" },
    });
    expect(sentEvents(0)).toEqual([
      {
        ts: "2026-10-25T20:00:00.000+08:00",
        action: "started",
        latency_ms: null,
        level: "info",
        error_message: null,
        context: { request_url: "https://app.example/home?a=1" },
        extra: { page: "/home" },
      },
      expect.objectContaining({
        level: "error",
        error_message: "it broke",
        action: "boom",
        extra: { page: "/other", code: 42 },
      }),
    ]);

    await vi.advanceTimersByTimeAsync(60_000);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    Logger.info("later");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(sentEvents(1)).toHaveLength(1);
  });

  it("sends the queue with keepalive when the page is hidden", async () => {
    const Logger = await loadLogger();
    Logger.warn("leaving");
    hidePage();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0][1].keepalive).toBe(true);

    await vi.advanceTimersByTimeAsync(60_000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("skips an empty message", async () => {
    const Logger = await loadLogger();
    Logger.info("  ");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("keeps the newest 1000 events", async () => {
    const Logger = await loadLogger();
    for (let index = 0; index < 1001; index++) Logger.info(`event ${index}`);
    await vi.advanceTimersByTimeAsync(60_000);
    const events = sentEvents(0);
    expect(events).toHaveLength(1000);
    expect(events[0].action).toBe("event 1");
  });

  it("masks the value of a matching extra key and each match in a string value", async () => {
    const Logger = await loadLogger();
    Logger.setMaskPatterns([/^(password|token)$/i, /secret=\S+/]);
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
      page: "/home",
      password: "***",
      TOKEN: "***",
      note: "a *** and ***",
      code: 7,
    });
  });

  it("keeps the extra values when no mask pattern is set", async () => {
    const Logger = await loadLogger();
    Logger.info("login", { password: "hunter2" });
    hidePage();
    expect(sentEvents(0)[0].extra).toEqual({ page: "/home", password: "hunter2" });
  });

  it("ignores a failed send", async () => {
    const Logger = await loadLogger();
    fetchMock.mockRejectedValueOnce(new Error("offline"));
    Logger.info("first");
    await vi.advanceTimersByTimeAsync(60_000);
    Logger.info("second");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(sentEvents(1)).toHaveLength(1);
  });

  it("records a window error and an unhandled rejection as error events", async () => {
    const windowListeners: Record<string, (event: unknown) => void> = {};
    vi.stubGlobal("window", {
      addEventListener(type: string, listener: (event: unknown) => void) {
        windowListeners[type] = listener;
      },
    });
    const Logger = await loadLogger();
    Logger.watchWindowErrors();
    windowListeners.error({ message: "boom" });
    windowListeners.error({ message: "" });
    windowListeners.unhandledrejection({ reason: new Error("rejected") });
    windowListeners.unhandledrejection({ reason: 42 });
    hidePage();
    const sent = sentEvents(0).map(
      (event: { level: string; action: string; error_message: string }) => [
        event.level,
        event.action,
        event.error_message,
      ],
    );
    expect(sent).toEqual([
      ["error", "WINDOW/error", "boom"],
      ["error", "WINDOW/error", "window error"],
      ["error", "WINDOW/unhandledrejection", "rejected"],
      ["error", "WINDOW/unhandledrejection", "unhandled rejection"],
    ]);
  });

  it("keeps a long window error message out of the action and masks it", async () => {
    const windowListeners: Record<string, (event: unknown) => void> = {};
    vi.stubGlobal("window", {
      addEventListener(type: string, listener: (event: unknown) => void) {
        windowListeners[type] = listener;
      },
    });
    const Logger = await loadLogger();
    Logger.setMaskPatterns([/secret-\d+/]);
    Logger.watchWindowErrors();
    windowListeners.error({ message: `bad secret-42 ${"x".repeat(5000)}` });
    hidePage();
    const [event] = sentEvents(0);
    expect(event.action).toBe("WINDOW/error");
    expect(event.error_message).toContain("bad *** ");
    expect(event.error_message).not.toContain("secret-42");
  });

  it("sends a fallback error message when the message is empty", async () => {
    const Logger = await loadLogger();
    Logger.error("a", "");
    Logger.error("b", "   ");
    const fail = Logger.wrapAction("c", () => {
      throw new Error("");
    });
    expect(() => fail()).toThrow();
    hidePage();
    const events = sentEvents(0);
    expect(events.map((event: { error_message: string }) => event.error_message)).toEqual([
      "unknown error",
      "unknown error",
      "unknown error",
    ]);
    expect(events[2].extra.error).toBe("unknown error");
  });

  describe("contextWrapper", () => {
    it("wraps each function with the action name module/function_name", async () => {
      const Logger = await loadLogger();
      const cart = Logger.contextWrapper("cart", {
        add: (a: number, b: number) => a + b,
        limit: 5,
      });

      expect(cart.add(1, 2)).toBe(3);
      expect(cart.limit).toBe(5);
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0].action).toBe("cart/add");
      expect(events[0].extra.args).toBe("[1,2]");
    });
  });

  describe("wrapAction", () => {
    it("records one info event with the latency and returns the result", async () => {
      const Logger = await loadLogger();
      const add = Logger.wrapAction("cart/add", (a: number, b: number) => {
        vi.advanceTimersByTime(25);
        return a + b;
      });

      expect(add(1, 2)).toBe(3);
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0].action).toBe("cart/add");
      expect(events[0].level).toBe("info");
      expect(events[0].latency_ms).toBe(25);
      expect(events[0].extra.args).toBe("[1,2]");
    });

    it("records one error event with the latency and rethrows when the action throws", async () => {
      const Logger = await loadLogger();
      const fail = Logger.wrapAction("cart/remove", () => {
        vi.advanceTimersByTime(10);
        throw new Error("boom");
      });

      expect(() => fail()).toThrow("boom");
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0].level).toBe("error");
      expect(events[0].latency_ms).toBe(10);
      expect(events[0].extra.error).toBe("boom");
      expect(events[0].error_message).toBe("boom");
      expect(events[0].extra.args).toBe("[]");
    });

    it("records unserializable args as a fixed text", async () => {
      const Logger = await loadLogger();
      const circular: Record<string, unknown> = {};
      circular.self = circular;
      Logger.wrapAction("cart/view", (_value: unknown) => undefined)(circular);
      vi.advanceTimersByTime(60_000);

      expect(sentEvents(0)[0].extra.args).toBe("unserializable");
    });

    it("records the event after the async action resolves", async () => {
      const Logger = await loadLogger();
      const save = Logger.wrapAction("cart/save", async () => {
        await new Promise((resolve) => setTimeout(resolve, 40));
        return "ok";
      });

      const pending = save();
      vi.advanceTimersByTime(40);
      await expect(pending).resolves.toBe("ok");
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0].level).toBe("info");
      expect(events[0].latency_ms).toBe(40);
    });

    it("records one error event when the async action rejects", async () => {
      const Logger = await loadLogger();
      const fail = Logger.wrapAction("cart/save", async () => {
        await new Promise((resolve) => setTimeout(resolve, 15));
        throw new Error("denied");
      });

      const pending = fail();
      const assertion = expect(pending).rejects.toThrow("denied");
      vi.advanceTimersByTime(15);
      await assertion;
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      expect(events).toHaveLength(1);
      expect(events[0].level).toBe("error");
      expect(events[0].latency_ms).toBe(15);
      expect(events[0].extra.error).toBe("denied");
      expect(events[0].error_message).toBe("denied");
    });
  });

  describe("context", () => {
    it("puts the added context in each later event", async () => {
      const Logger = await loadLogger();
      Logger.info("before");
      Logger.addContext("user", "u1");
      Logger.addContext("beta", "on");
      Logger.addContext("user", "u2");
      Logger.info("after");
      vi.advanceTimersByTime(60_000);

      const events = sentEvents(0);
      const request_url = "https://app.example/home?a=1";
      expect(events[0].context).toEqual({ request_url });
      expect(events[1].context).toEqual({ request_url, user: "u2", beta: "on" });
    });

    it("removes the named keys", async () => {
      const Logger = await loadLogger();
      Logger.addContext("user", "u1");
      Logger.addContext("beta", "on");
      Logger.removeContext("user", "missing");
      Logger.info("after");
      vi.advanceTimersByTime(60_000);

      expect(sentEvents(0)[0].context).toEqual({
        request_url: "https://app.example/home?a=1",
        beta: "on",
      });
    });

    it("masks each match in the request url", async () => {
      const Logger = await loadLogger();
      Logger.setMaskPatterns([/a=\d+/]);
      Logger.info("page");
      vi.advanceTimersByTime(60_000);

      expect(sentEvents(0)[0].context).toEqual({ request_url: "https://app.example/home?***" });
    });
  });
});
