import { hktIsoString } from "../util/time";
import { Mask } from "./mask";
import type { ClientLogEvent, LogFields, LogLevel } from "./types";

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

  public static info(action: string, extra: LogFields = {}): void {
    Logger.buffer.push({
      ts: hktIsoString(new Date()),
      action,
      latencyMs: null,
      level: "info",
      errorMessage: null,
      context: { ...Logger.context },
      extra: Logger.mask.fields(extra),
    });
  }

  public static warn(action: string, extra: LogFields = {}, error?: Error): void {
    Logger.buffer.push({
      ts: hktIsoString(new Date()),
      action,
      latencyMs: null,
      level: "warn",
      errorMessage: Logger.errorMessage("warn", error),
      context: { ...Logger.context },
      extra: Logger.mask.fields(extra),
    });
  }

  public static error(action: string, error: Error, extra: LogFields = {}): void {
    Logger.buffer.push({
      ts: hktIsoString(new Date()),
      action,
      latencyMs: null,
      level: "error",
      errorMessage: Logger.errorMessage("error", error),
      context: { ...Logger.context },
      extra: Logger.mask.fields(extra),
    });
  }

  public static async capture<T>(actionName: string, work: (extra: LogFields) => Promise<T>): Promise<T> {
    const start = performance.now();
    const extra: LogFields = {};
    try {
      const output = await work(extra);
      Logger.buffer.push({
        ts: hktIsoString(new Date()),
        action: actionName,
        latencyMs: Math.round(performance.now() - start),
        level: "info",
        errorMessage: null,
        context: { ...Logger.context },
        extra: Logger.mask.fields(extra),
      });
      return output;
    } catch (error) {
      const errorMessage = error instanceof Error ? Logger.errorMessage("error", error) : UNKNOWN_ERROR;
      Logger.buffer.push({
        ts: hktIsoString(new Date()),
        action: actionName,
        latencyMs: Math.round(performance.now() - start),
        level: "error",
        errorMessage,
        context: { ...Logger.context },
        extra: Logger.mask.fields(extra),
      });
      throw error;
    }
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
          Logger.error("WINDOW/error", event.error);
          return;
        }
        const message = event.message || Logger.domSourceMessage(event.target) || "window error";
        Logger.error("WINDOW/error", new Error(message));
      },
      true,
    );
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      const error = reason instanceof Error ? reason : new Error("unhandled rejection");
      Logger.error("WINDOW/unhandledrejection", error);
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
