import { hktIsoString } from "./time";

const LOG_URL = "/log";
const FLUSH_INTERVAL = 60_000;
const MAX_QUEUE = 1000;
const MASK = "***";
const UNKNOWN_ERROR = "unknown error";
const DUPLICATE_ERROR_WINDOW = 5000;
const SLOW_STARTUP_THRESHOLD = 5000;

export type LogLevel = "info" | "warn" | "error";
export type LogFields = Record<string, string | number | boolean>;

type ClientLogEvent = {
  ts: string;
  action: string;
  latency_ms: number | null;
  level: LogLevel;
  error_message: string | null;
  context: LogFields;
  extra: LogFields;
};

export type LogScope = {
  addInfo(key: string, value: string | number | boolean): void;
};

class ActionScope implements LogScope {
  private readonly info: LogFields = {};
  private closed = false;

  addInfo(key: string, value: string | number | boolean): void {
    if (this.closed) {
      console.warn(`scope.addInfo(${key}) is ignored, because the action is out of its scope`);
    } else {
      this.info[key] = value;
    }
  }

  close(): LogFields {
    this.closed = true;
    return this.info;
  }
}

const OUT_OF_SCOPE = new ActionScope();
OUT_OF_SCOPE.close();

export class Logger {
  private static queue: ClientLogEvent[] = [];
  private static scopes: ActionScope[] = [];
  private static sending = false;
  private static started = false;
  private static masks: RegExp[] = [];
  private static context: LogFields = {};

  public static setMaskPatterns(patterns: RegExp[]): void {
    Logger.masks = patterns.map(
      (pattern) =>
        new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`),
    );
  }

  public static addContext(key: string, value: string | number | boolean): void {
    Logger.context = { ...Logger.context, [key]: value };
  }

  public static removeContext(...keys: string[]): void {
    const kept = { ...Logger.context };
    for (const key of keys) delete kept[key];
    Logger.context = kept;
  }

  public static watchWindowErrors(): void {
    // The capture phase is the only way to see a failed script, style or image load, as these events do not bubble.
    window.addEventListener(
      "error",
      (event) => {
        const message = event.message || Logger.domSourceMessage(event.target) || "window error";
        Logger.error("WINDOW/error", message);
      },
      true,
    );
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      const message = reason instanceof Error ? reason.message : "unhandled rejection";
      Logger.error("WINDOW/unhandledrejection", message);
    });
  }

  // Returns the scope of the running wrapped action. Call it before the first `await` of the action.
  // The scope can be used after an `await`, until the action ends.
  public static scope(): LogScope {
    return Logger.scopes[Logger.scopes.length - 1] ?? OUT_OF_SCOPE;
  }

  public static watchStartupPerformance(): void {
    const log = () => setTimeout(Logger.recordStartupPerformance, 0);
    if (document.readyState === "complete") {
      log();
    } else {
      window.addEventListener("load", log, { once: true });
    }
  }

  public static info(action: string, extra?: LogFields): void {
    Logger.record("info", action, null, extra);
  }

  public static warn(action: string, extra?: LogFields): void {
    Logger.record("warn", action, null, extra);
  }

  public static error(action: string, errorMessage: string, extra?: LogFields): void {
    Logger.record("error", action, errorMessage, extra);
  }

  public static wrapAction<Args extends unknown[], Result>(
    name: string,
    action: (...args: Args) => Result,
  ): (...args: Args) => Result {
    return (...args) => {
      const start = performance.now();
      const argsExtra = Logger.argsExtra(args);
      const scope = new ActionScope();
      const succeed = () =>
        Logger.record("info", name, null, { ...scope.close(), ...argsExtra }, performance.now() - start);
      const fail = (error: unknown) => {
        const failure = Logger.failure(error);
        Logger.record(
          "error",
          name,
          failure.error,
          { ...scope.close(), ...argsExtra, ...failure },
          performance.now() - start,
        );
      };

      let result: Result;
      Logger.scopes.push(scope);
      try {
        result = action(...args);
      } catch (error) {
        fail(error);
        throw error;
      } finally {
        Logger.scopes.pop(); // only the sync part of the action runs here
      }
      if (!(result instanceof Promise)) {
        succeed();
        return result;
      }
      return result.then(
        (value) => {
          succeed();
          return value;
        },
        (error: unknown) => {
          fail(error);
          throw error;
        },
      ) as Result;
    };
  }

  public static contextWrapper<Context extends Record<string, unknown>>(
    module: string,
    context: Context,
  ): Context {
    const wrapped: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(context)) {
      wrapped[key] =
        typeof value === "function"
          ? Logger.wrapAction(`${module}/${key}`, value as (...args: unknown[]) => unknown)
          : value;
    }
    return wrapped as Context;
  }

  private static domSourceMessage(target: EventTarget | null): string | null {
    if (typeof Element === "undefined" || !(target instanceof Element)) return null;
    const source = target.getAttribute("src") ?? target.getAttribute("href") ?? "";
    return `DOM source error: <${target.tagName.toLowerCase()}> ${source}`.trim();
  }

  private static argsExtra(args: unknown[]): LogFields {
    try {
      return { args: JSON.stringify(args) };
    } catch {
      return { args: "unserializable" };
    }
  }

  private static failure(error: unknown): { error: string } {
    const message = error instanceof Error ? error.message : "";
    return { error: message.trim() === "" ? UNKNOWN_ERROR : message };
  }

  private static record(
    level: LogLevel,
    action: string,
    errorMessage: string | null,
    extra?: LogFields,
    latencyMs?: number,
  ): void {
    if (action.trim() === "") return;
    Logger.start();

    const event: ClientLogEvent = {
      ts: hktIsoString(new Date()),
      action,
      latency_ms: latencyMs === undefined ? null : Math.round(latencyMs),
      level,
      error_message: Logger.errorMessage(level, errorMessage),
      context: { request_url: Logger.maskText(location.href), ...Logger.context },
      extra: Logger.mask({ page: location.pathname, ...extra }),
    };
    if (Logger.isRepeatedError(event)) return;

    if (Logger.queue.length >= MAX_QUEUE) Logger.queue.shift();
    Logger.queue.push(event);
  }

  // A loop can make the same error many times. The queue keeps one event for the same action and message in 5 seconds.
  private static isRepeatedError(event: ClientLogEvent): boolean {
    const last = Logger.queue[Logger.queue.length - 1];
    return (
      event.level === "error" &&
      last !== undefined &&
      last.level === "error" &&
      last.action === event.action &&
      last.error_message === event.error_message &&
      Date.parse(event.ts) - Date.parse(last.ts) <= DUPLICATE_ERROR_WINDOW
    );
  }

  private static recordStartupPerformance(): void {
    const timing = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
    // loadEventEnd is zero until the load handlers return
    if (!timing || timing.loadEventEnd <= 0) return;

    const elapsed = Math.round(timing.loadEventEnd - timing.startTime);
    const extra = {
      navigation_type: timing.type,
      http_start: Math.round(timing.requestStart),
      http_end: Math.round(timing.responseEnd),
      dom_start: Math.round(timing.domInteractive),
      dom_end: Math.round(timing.domComplete),
    };
    const level = elapsed >= SLOW_STARTUP_THRESHOLD ? "warn" : "info";
    Logger.record(level, "@@framework/startup-performance", null, extra, elapsed);
  }

  private static errorMessage(level: LogLevel, errorMessage: string | null): string | null {
    if (level === "error") {
      const text = errorMessage === null || errorMessage.trim() === "" ? UNKNOWN_ERROR : errorMessage;
      return Logger.maskText(text);
    }
    return errorMessage === null ? null : Logger.maskText(errorMessage);
  }

  private static maskText(text: string): string {
    return Logger.masks.reduce((masked, mask) => masked.replace(mask, MASK), text);
  }

  private static mask(extra: LogFields): LogFields {
    if (Logger.masks.length === 0) return extra;

    const masked: LogFields = {};
    for (const [key, value] of Object.entries(extra)) {
      if (Logger.masks.some((mask) => key.search(mask) !== -1)) {
        masked[key] = MASK;
      } else if (typeof value === "string") {
        masked[key] = Logger.maskText(value);
      } else {
        masked[key] = value;
      }
    }
    return masked;
  }

  private static start(): void {
    if (Logger.started) return;
    Logger.started = true;
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") Logger.flush(true);
    });
    if (typeof window !== "undefined") window.addEventListener("pagehide", () => Logger.beacon());
    setInterval(() => Logger.flush(false), FLUSH_INTERVAL);
  }

  // A beacon is sent even when the page is closing. It cannot be read back, so a failed beacon loses the events.
  private static beacon(): void {
    if (Logger.queue.length === 0) return;
    if (typeof navigator === "undefined" || typeof navigator.sendBeacon !== "function") {
      Logger.flush(true);
      return;
    }

    const events = Logger.queue;
    Logger.queue = [];
    const body = new Blob([JSON.stringify({ events })], { type: "application/json" });
    if (!navigator.sendBeacon(LOG_URL, body)) Logger.restore(events);
  }

  private static async flush(keepalive: boolean): Promise<void> {
    if (Logger.queue.length === 0 || Logger.sending) return;

    const events = Logger.queue;
    Logger.queue = [];
    Logger.sending = true;
    try {
      const response = await fetch(LOG_URL, {
        method: "POST",
        credentials: "same-origin",
        keepalive,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ events }),
      });
      // The status 400 to 405 fails again with the same events, so the events are dropped. Other failures send again later.
      const isRejected = response.status >= 400 && response.status <= 405;
      if (!response.ok && !isRejected) Logger.restore(events);
    } catch {
      Logger.restore(events);
    } finally {
      Logger.sending = false;
    }
  }

  private static restore(events: ClientLogEvent[]): void {
    Logger.queue = [...events, ...Logger.queue].slice(-MAX_QUEUE);
  }
}
