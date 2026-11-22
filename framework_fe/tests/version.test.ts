import {afterEach, beforeEach, describe, expect, it, vi} from "vitest";

type FakeDocument = {
    visibilityState: DocumentVisibilityState;
    listeners: (() => void)[];
    addEventListener: (type: string, listener: () => void) => void;
};

let fakeDocument: FakeDocument;
let fetchMock: ReturnType<typeof vi.fn>;

function respondWith(version: string): void {
    fetchMock.mockResolvedValueOnce({ok: true, json: async () => ({version})});
}

function showPage(): void {
    fakeDocument.visibilityState = "visible";
    fakeDocument.listeners.forEach(listener => {
        listener();
    });
}

async function flush(): Promise<void> {
    await new Promise(resolve => setTimeout(resolve, 0));
}

async function loadChecker() {
    const {VersionChecker} = await import("../src/version");
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
    vi.useRealTimers();
    vi.unstubAllGlobals();
});

describe("VersionChecker", () => {
    it("calls the handler when the version changes", async () => {
        const VersionChecker = await loadChecker();
        const handler = vi.fn();

        respondWith("a");
        VersionChecker.init({onUpdated: handler});
        await flush();
        expect(fetchMock).toHaveBeenCalledWith("/version", {
            method: "GET",
            credentials: "same-origin",
            cache: "no-store",
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

    it("calls the handler on each check that gets a new version", async () => {
        const VersionChecker = await loadChecker();
        const handler = vi.fn();

        respondWith("a");
        VersionChecker.init({onUpdated: handler});
        await flush();

        for (const version of ["b", "b", "a", "c"]) {
            respondWith(version);
            showPage();
            await flush();
        }
        expect(handler).toHaveBeenCalledTimes(3);
    });

    it("ignores a body that has no version text", async () => {
        const VersionChecker = await loadChecker();
        const handler = vi.fn();

        fetchMock.mockResolvedValueOnce({ok: true, json: async () => ({})});
        VersionChecker.init({onUpdated: handler});
        await flush();
        respondWith("a");
        showPage();
        await flush();
        for (const body of [{}, null, {version: 2}]) {
            fetchMock.mockResolvedValueOnce({ok: true, json: async () => body});
            showPage();
            await flush();
        }
        expect(handler).not.toHaveBeenCalled();

        respondWith("b");
        showPage();
        await flush();
        expect(handler).toHaveBeenCalledTimes(1);
    });

    it("throws when init is called twice", async () => {
        const VersionChecker = await loadChecker();
        respondWith("a");
        VersionChecker.init({onUpdated: vi.fn()});
        expect(() => VersionChecker.init({onUpdated: vi.fn()})).toThrow("called twice");
        await flush();
    });

    it("checks only when the page becomes visible", async () => {
        const VersionChecker = await loadChecker();
        respondWith("a");
        VersionChecker.init({onUpdated: vi.fn()});
        await flush();
        expect(fetchMock).toHaveBeenCalledTimes(1);

        fakeDocument.visibilityState = "hidden";
        fakeDocument.listeners.forEach(listener => {
            listener();
        });
        await flush();
        expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    it("ignores failed checks", async () => {
        const VersionChecker = await loadChecker();
        const handler = vi.fn();

        respondWith("a");
        VersionChecker.init({onUpdated: handler});
        await flush();

        fetchMock.mockRejectedValueOnce(new Error("offline"));
        showPage();
        await flush();
        fetchMock.mockResolvedValueOnce({ok: false});
        showPage();
        await flush();
        expect(handler).not.toHaveBeenCalled();
    });

    it("checks on the interval when the page is hidden", async () => {
        vi.useFakeTimers();
        const VersionChecker = await loadChecker();
        const handler = vi.fn();

        respondWith("a");
        VersionChecker.init({onUpdated: handler, checkInterval: 1000});
        await vi.advanceTimersByTimeAsync(0);
        expect(fetchMock).toHaveBeenCalledTimes(1);

        respondWith("b");
        await vi.advanceTimersByTimeAsync(1000);
        expect(fetchMock).toHaveBeenCalledTimes(2);
        expect(handler).toHaveBeenCalledTimes(1);

        fakeDocument.visibilityState = "hidden";
        respondWith("b");
        await vi.advanceTimersByTimeAsync(1000);
        expect(fetchMock).toHaveBeenCalledTimes(3);
    });

    it("does not check on a timer without an interval", async () => {
        vi.useFakeTimers();
        const VersionChecker = await loadChecker();

        respondWith("a");
        VersionChecker.init({onUpdated: vi.fn()});
        await vi.advanceTimersByTimeAsync(60_000);
        expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    it("reload bypasses the cache before it reloads", async () => {
        const VersionChecker = await loadChecker();
        const reload = vi.fn();
        vi.stubGlobal("location", {href: "/folder?x=1", reload});

        fetchMock.mockRejectedValueOnce(new Error("offline"));
        await VersionChecker.reload();
        expect(fetchMock).toHaveBeenLastCalledWith("/folder?x=1", {cache: "reload"});
        expect(reload).toHaveBeenCalledTimes(1);
    });
});
