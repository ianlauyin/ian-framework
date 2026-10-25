import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type FakeDocument = {
  visibilityState: DocumentVisibilityState;
  listeners: (() => void)[];
  addEventListener: (type: string, listener: () => void) => void;
};

let fakeDocument: FakeDocument;
let fetchMock: ReturnType<typeof vi.fn>;

function respondWith(version: string): void {
  fetchMock.mockResolvedValueOnce({ ok: true, json: async () => ({ version }) });
}

function showPage(): void {
  fakeDocument.visibilityState = "visible";
  fakeDocument.listeners.forEach((listener) => listener());
}

async function flush(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function loadChecker() {
  const { VersionChecker } = await import("./version");
  return VersionChecker;
}

beforeEach(() => {
  vi.resetModules();
  fakeDocument = {
    visibilityState: "visible",
    listeners: [],
    addEventListener(type, listener) {
      if (type === "visibilitychange") fakeDocument.listeners.push(listener);
    },
  };
  fetchMock = vi.fn();
  vi.stubGlobal("document", fakeDocument);
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("VersionChecker", () => {
  it("calls the handler when the version changes", async () => {
    const VersionChecker = await loadChecker();
    const handler = vi.fn();
    VersionChecker.setUpdatedHandler(handler);

    respondWith("a");
    VersionChecker.init();
    await flush();
    expect(fetchMock).toHaveBeenCalledWith("/version", {
      method: "GET",
      credentials: "same-origin",
    });

    respondWith("a");
    showPage();
    await flush();
    expect(handler).not.toHaveBeenCalled();

    respondWith("b");
    showPage();
    await flush();
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("checks only when the page becomes visible", async () => {
    const VersionChecker = await loadChecker();
    respondWith("a");
    VersionChecker.init();
    VersionChecker.init();
    await flush();
    expect(fetchMock).toHaveBeenCalledTimes(1);

    fakeDocument.visibilityState = "hidden";
    fakeDocument.listeners.forEach((listener) => listener());
    await flush();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("ignores failed checks", async () => {
    const VersionChecker = await loadChecker();
    const handler = vi.fn();
    VersionChecker.setUpdatedHandler(handler);

    respondWith("a");
    VersionChecker.init();
    await flush();

    fetchMock.mockRejectedValueOnce(new Error("offline"));
    showPage();
    await flush();
    fetchMock.mockResolvedValueOnce({ ok: false });
    showPage();
    await flush();
    expect(handler).not.toHaveBeenCalled();
  });
});
