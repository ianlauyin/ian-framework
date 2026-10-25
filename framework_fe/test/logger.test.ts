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
  fakeDocument.listeners.forEach((listener) => listener());
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
  vi.stubGlobal("location", { pathname: "/home" });
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
    Logger.error("boom", { code: 42, page: "/other" });
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
        ts: "2026-10-25T12:00:00.000Z",
        id: "",
        ip: "",
        user_agent: "",
        action: "",
        status: null,
        latency_ms: null,
        level: "info",
        message: "started",
        extra: { page: "/home" },
      },
      expect.objectContaining({
        level: "error",
        message: "boom",
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
    expect(events[0].message).toBe("event 1");
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
});
