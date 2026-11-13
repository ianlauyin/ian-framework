import { hktIsoString } from "../util/time";
import { LogBuffer } from "./log-buffer";
import { HttpLogShipper } from "./log-shipper";
import { Mask } from "./mask";
import type { LogEvent, LogFields, LogLevel } from "./types";

const FLUSH_INTERVAL = 60_000;
const UNKNOWN_ERROR = "unknown error";

export class Logger {
  private static shipper = new HttpLogShipper();
  private static buffer = new LogBuffer();
  private static sending = false;
  private static started = false;
  private static mask = new Mask([]);
  private static context: LogFields = {};

  public static setMaskPatterns(patterns: RegExp[]): void {
    Logger.mask = new Mask(patterns);
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
        Logger.error("WINDOW/error", {}, message);
      },
      true,
    );
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      const message = reason instanceof Error ? reason.message : "unhandled rejection";
      Logger.error("WINDOW/unhandledrejection", {}, message);
    });
  }

  public static info(action: string, context: LogFields = {}): void {
    Logger.record({ level: "info", action, context });
  }

  public static warn(action: string, context: LogFields = {}, error?: unknown): void {
    Logger.record({
      level: "warn",
      action,
      context,
      errorMessage: error === undefined ? null : Logger.errorText(error),
    });
  }

  public static error(action: string, context: LogFields, error: unknown): void {
    Logger.record({ level: "error", action, context, errorMessage: Logger.errorText(error) });
  }

  private static domSourceMessage(target: EventTarget | null): string | null {
    if (typeof Element === "undefined" || !(target instanceof Element)) return null;
    const source = target.getAttribute("src") ?? target.getAttribute("href") ?? "";
    return `DOM source error: <${target.tagName.toLowerCase()}> ${source}`.trim();
  }

  private static errorText(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  public static record(event: LogEvent): void {
    if (event.action.trim() === "") return;

    Logger.start();
    Logger.buffer.push({
      ts: hktIsoString(new Date()),
      action: event.action,
      latency_ms: event.latencyMs === undefined ? null : Math.round(event.latencyMs),
      level: event.level,
      error_message: Logger.errorMessage(event.level, event.errorMessage ?? null),
      context: { request_url: Logger.mask.text(location.href), ...Logger.context, ...Logger.mask.fields(event.context ?? {}) },
      extra: Logger.mask.fields({ page: location.pathname, ...event.extra }),
    });
  }

  private static errorMessage(level: LogLevel, errorMessage: string | null): string | null {
    if (level === "error") {
      const text = errorMessage === null || errorMessage.trim() === "" ? UNKNOWN_ERROR : errorMessage;
      return Logger.mask.text(text);
    }
    return errorMessage === null ? null : Logger.mask.text(errorMessage);
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
