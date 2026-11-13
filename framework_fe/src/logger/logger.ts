import { hktIsoString } from "../util/time";
import { maskFields, maskText, setMaskPatterns } from "./mask";
import { enqueue } from "./transport";
import type { LogEvent, LogFields, LogLevel } from "./types";

const UNKNOWN_ERROR = "unknown error";

export class Logger {
  private static context: LogFields = {};

  public static setMaskPatterns(patterns: RegExp[]): void {
    setMaskPatterns(patterns);
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

    enqueue({
      ts: hktIsoString(new Date()),
      action: event.action,
      latency_ms: event.latencyMs === undefined ? null : Math.round(event.latencyMs),
      level: event.level,
      error_message: Logger.errorMessage(event.level, event.errorMessage ?? null),
      context: { request_url: maskText(location.href), ...Logger.context, ...maskFields(event.context ?? {}) },
      extra: maskFields({ page: location.pathname, ...event.extra }),
    });
  }

  private static errorMessage(level: LogLevel, errorMessage: string | null): string | null {
    if (level === "error") {
      const text = errorMessage === null || errorMessage.trim() === "" ? UNKNOWN_ERROR : errorMessage;
      return maskText(text);
    }
    return errorMessage === null ? null : maskText(errorMessage);
  }
}
