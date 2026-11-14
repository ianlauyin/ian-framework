import { hktIsoString } from "../util/time";
import { Mask } from "./mask";
import type { CaptureEvent, ClientLogEvent, LogFields, LogLevel } from "./types";

const LOG_URL = "/log";
const SEND_INTERVAL = 60_000;
const UNKNOWN_ERROR = "unknown error";

export class Logger {
  private static initialized = false;
  private static buffer: ClientLogEvent[] = [];
  private static mask = new Mask([]);
  private static context: LogFields = {};

  public static init(patterns: RegExp[]): void {
    if (Logger.initialized) return;
    Logger.initialized = true;
    Logger.mask = new Mask(patterns);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") Logger.send();
    });
    if (typeof window !== "undefined") {
      window.addEventListener("pagehide", () => Logger.send());
      Logger.watchWindowError();
    }
    setInterval(() => Logger.send(), SEND_INTERVAL);
  }

  public static addContext(key: string, value: string | number | boolean): void {
    Logger.context = { ...Logger.context, [key]: value };
  }

  public static removeContext(...keys: string[]): void {
    const kept = { ...Logger.context };
    for (const key of keys) delete kept[key];
    Logger.context = kept;
  }

  public static info(action: string, context: LogFields = {}): void {
    Logger.record("info", action, context, undefined, null);
  }

  public static warn(action: string, context: LogFields = {}, error?: Error): void {
    Logger.record("warn", action, context, error, null);
  }

  public static error(action: string, context: LogFields, error: Error): void {
    Logger.record("error", action, context, error, null);
  }

  public static async capture<T>(
    actionName: string,
    work: (event: CaptureEvent) => Promise<T>,
  ): Promise<T> {
    const start = performance.now();
    const captureEvent = {
      level: "info",
      extra: {},
      errorMessage: null
    };
    try {
      let result = await work(captureEvent);
      Logger.buffer.push({
        ts,
        action: actionName,
        latencyMs: Math.round(performance.now() - start),
        level: "info",
        errorMessage: null,
        context: { request_url: Logger.mask.text(location.href), ...Logger.context },
        extra: Logger.mask.fields({ page: location.pathname, ...event.extra }),
      })
      return result
    } catch (error) {
      level = "error";
      const message = error instanceof Error ? error.message : "";
      errorMessage = message.trim() === "" ? UNKNOWN_ERROR : message;
      Logger.buffer.push({
        ts: event.ts,
        action: actionName,
        latencyMs: Math.round(performance.now() - start),
        level,
        errorMessage: errorMessage === null ? null : Logger.mask.text(errorMessage),
        context: { request_url: Logger.mask.text(location.href), ...Logger.context },
        extra: Logger.mask.fields({ page: location.pathname, ...event.extra }),
      })
      throw error;
    }
  }

  private static record(
    level: LogLevel,
    action: string,
    context: LogFields,
    error: Error | undefined,
    latencyMs: number | null,
  ): void {
    if (action.trim() === "") return;

    Logger.buffer.push({
      ts: hktIsoString(new Date()),
      action,
      latencyMs: latencyMs === null ? null : Math.round(latencyMs),
      level,
      errorMessage: Logger.errorMessage(level, error),
      context: { request_url: Logger.mask.text(location.href), ...Logger.context, ...Logger.mask.fields(context) },
      extra: Logger.mask.fields({ page: location.pathname }),
    });
  }

  private static errorMessage(level: LogLevel, error: Error | undefined): string | null {
    if (level === "info" || error === undefined) return null;
    const text = level === "error" && error.message.trim() === "" ? UNKNOWN_ERROR : error.message;
    return Logger.mask.text(text);
  }

  private static watchWindowError(): void {
    // The capture phase is the only way to see a failed script, style or image load, as these events do not bubble.
    window.addEventListener(
      "error",
      (event) => {
        if (event.error instanceof Error) {
          Logger.error("WINDOW/error", {}, event.error);
          return;
        }
        const message = event.message || Logger.domSourceMessage(event.target) || "window error";
        Logger.error("WINDOW/error", {}, new Error(message));
      },
      true,
    );
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      const error = reason instanceof Error ? reason : new Error("unhandled rejection");
      Logger.error("WINDOW/unhandledrejection", {}, error);
    });
  }

  private static domSourceMessage(target: EventTarget | null): string | null {
    if (typeof Element === "undefined" || !(target instanceof Element)) return null;
    const source = target.getAttribute("src") ?? target.getAttribute("href") ?? "";
    return `DOM source error: <${target.tagName.toLowerCase()}> ${source}`.trim();
  }

  // A beacon cannot be read back, so only a beacon that the browser refuses is put back in the buffer.
  private static send(): void {
    if (Logger.buffer.length === 0) return;

    const events = Logger.buffer;
    Logger.buffer = [];
    const body = new Blob([JSON.stringify({ events })], { type: "application/json" });
    if (!navigator.sendBeacon(LOG_URL, body)) Logger.buffer = [...events, ...Logger.buffer];
  }
}
