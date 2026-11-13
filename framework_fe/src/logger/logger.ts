import { hktIsoString } from "../util/time";
import { LogBuffer } from "./log-buffer";
import { HttpLogShipper } from "./log-shipper";
import { Mask } from "./mask";
import type { LogFields, LogLevel } from "./types";

const FLUSH_INTERVAL = 60_000;
const UNKNOWN_ERROR = "unknown error";

type CaptureCall =
  | { level: "info"; action: string; context?: LogFields }
  | { level: "warn"; action: string; context?: LogFields; error?: Error }
  | { level: "error"; action: string; context?: LogFields; error: Error };

export class Logger {
  private static shipper = new HttpLogShipper();
  private static buffer = new LogBuffer();
  private static sending = false;
  private static initialized = false;
  private static mask = new Mask([]);
  private static context: LogFields = {};

  public static init(patterns: RegExp[]): void {
    if (Logger.initialized) return;
    Logger.initialized = true;
    Logger.mask = new Mask(patterns);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") Logger.flush(true);
    });
    if (typeof window !== "undefined") {
      window.addEventListener("pagehide", () => Logger.beacon());
      Logger.watchWindowError();
    }
    setInterval(() => Logger.flush(false), FLUSH_INTERVAL);
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

  public static async capture<T>(work: Promise<T>, toLog: (output: T) => CaptureCall): Promise<T> {
    const start = performance.now();
    const output = await work;
    const call = toLog(output);
    const latencyMs = performance.now() - start;
    const error = call.level === "info" ? undefined : call.error;
    Logger.record(call.level, call.action, call.context ?? {}, error, latencyMs);
    return output;
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
      latency_ms: latencyMs === null ? null : Math.round(latencyMs),
      level,
      error_message: Logger.errorMessage(level, error),
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

  private static beacon(): void {
    if (Logger.buffer.len() === 0) return;
    if (!Logger.shipper.canBeacon()) {
      Logger.flush(true);
      return;
    }

    const events = Logger.buffer.take();
    if (!Logger.shipper.beacon(events)) Logger.buffer.restore(events);
  }

  private static async flush(keepalive: boolean): Promise<void> {
    if (Logger.buffer.len() === 0 || Logger.sending) return;

    const events = Logger.buffer.take();
    Logger.sending = true;
    try {
      if (!(await Logger.shipper.ship(events, keepalive))) Logger.buffer.restore(events);
    } finally {
      Logger.sending = false;
    }
  }
}

export const capture = Logger.capture;
