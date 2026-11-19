// @vitest-environment happy-dom

import type { JSX } from "@solidjs/web";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type LoggedEvent = {
  action: string;
  context: { requestUrl?: string };
  extra: { from?: string; to?: string };
};

type Navigate = (path: string) => void;

let sendBeacon: ReturnType<typeof vi.fn>;
let dispose: (() => void) | undefined;
let go: Navigate | undefined;

function Shell(props: { NavigationLogger: () => JSX.Element; useNavigate: () => Navigate }) {
  const NavigationLogger = props.NavigationLogger;
  const navigate = props.useNavigate();
  go = navigate;
  return <NavigationLogger />;
}

async function load() {
  const [{ NavigationLogger }, { Logger }, router, web] = await Promise.all([
    import("../../src/solid/NavigationLogger"),
    import("../../src/logger"),
    import("@solidjs/router"),
    import("@solidjs/web"),
  ]);
  return {
    NavigationLogger,
    Logger,
    createRouter: router.createRouter,
    useNavigate: router.useNavigate,
    render: web.render,
  };
}

async function flushEvents(): Promise<LoggedEvent[]> {
  await vi.advanceTimersByTimeAsync(60_000);
  const body = sendBeacon.mock.calls.at(-1)?.[1] as Blob | undefined;
  if (!body) return [];
  const text = await body.text();
  return JSON.parse(text).events;
}

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-10-25T12:00:00Z"));
  sendBeacon = vi.fn().mockReturnValue(true);
  vi.stubGlobal("navigator", { sendBeacon });
  window.history.pushState({}, "", "/home?a=1#part");
  go = undefined;
});

afterEach(() => {
  dispose?.();
  dispose = undefined;
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("NavigationLogger", () => {
  it("does not record the first page and stores the page URL", async () => {
    const { NavigationLogger, Logger, createRouter, useNavigate, render } = await load();
    Logger.init([]);
    const Router = createRouter({
      routes: [{ path: "/home", component: () => null }],
    });
    const root = document.createElement("div");
    dispose = render(
      () => (
        <Router>{() => <Shell NavigationLogger={NavigationLogger} useNavigate={useNavigate} />}</Router>
      ),
      root,
    );
    Logger.info("later");

    const events = await flushEvents();
    expect(events.map((event) => event.action)).not.toContain("PAGE/switch");
    const later = events.find((event) => event.action === "later");
    expect(later?.context.requestUrl).toBe(`${window.location.origin}/home?a=1#part`);
  });

  it("records a pathname change", async () => {
    const { NavigationLogger, Logger, createRouter, useNavigate, render } = await load();
    Logger.init([]);
    const Router = createRouter({
      routes: [
        { path: "/home", component: () => null },
        { path: "/next", component: () => null },
      ],
    });
    const root = document.createElement("div");
    dispose = render(
      () => (
        <Router>{() => <Shell NavigationLogger={NavigationLogger} useNavigate={useNavigate} />}</Router>
      ),
      root,
    );
    go?.("/next");

    const events = await flushEvents();
    const change = events.find((event) => event.action === "PAGE/switch");
    expect(change?.extra).toEqual({ from: "/home", to: "/next" });
  });
});
