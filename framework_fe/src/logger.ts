import { hktIsoString } from "./time";

const LOG_URL = "/log";
const FLUSH_INTERVAL = 60_000;
const MAX_QUEUE = 1000;
const MASK = "***";

export type LogLevel = "info" | "warn" | "error";
export type LogFields = Record<string, string | number | boolean>;

type ClientLogEvent = {
  ts: string;
  id: string;
  action: string;
  latency_ms: null;
  level: LogLevel;
  context: LogFields;
  info: LogFields;
};

export class Logger {
  private static queue: ClientLogEvent[] = [];
  private static started = false;
  private static masks: RegExp[] = [];

  public static setMaskPatterns(patterns: RegExp[]): void {
    Logger.masks = patterns.map(
      (pattern) =>
        new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`),
    );
  }

  public static watchWindowErrors(): void {
    window.addEventListener("error", (event) => {
      Logger.error(event.message || "window error");
    });
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      Logger.error(reason instanceof Error ? reason.message : "unhandled rejection");
    });
  }

  public static info(message: string, info?: LogFields): void {
    Logger.record("info", message, info);
  }

  public static warn(message: string, info?: LogFields): void {
    Logger.record("warn", message, info);
  }

  public static error(message: string, info?: LogFields): void {
    Logger.record("error", message, info);
  }

  private static record(level: LogLevel, message: string, info?: LogFields): void {
    if (message.trim() === "") return;
    Logger.start();

    if (Logger.queue.length >= MAX_QUEUE) Logger.queue.shift();
    Logger.queue.push({
      ts: hktIsoString(new Date()),
      id: "",
      action: message,
      latency_ms: null,
      level,
      context: {},
      info: Logger.mask({ page: location.pathname, ...info }),
    });
  }

  private static mask(info: LogFields): LogFields {
    if (Logger.masks.length === 0) return info;

    const masked: LogFields = {};
    for (const [key, value] of Object.entries(info)) {
      if (Logger.masks.some((mask) => key.search(mask) !== -1)) {
        masked[key] = MASK;
      } else if (typeof value === "string") {
        masked[key] = Logger.masks.reduce((text, mask) => text.replace(mask, MASK), value);
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
    setInterval(() => Logger.flush(false), FLUSH_INTERVAL);
  }

  private static flush(keepalive: boolean): void {
    if (Logger.queue.length === 0) return;

    const events = Logger.queue;
    Logger.queue = [];
    fetch(LOG_URL, {
      method: "POST",
      credentials: "same-origin",
      keepalive,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ events }),
    }).catch(() => {
      // ignore failures — logging is best-effort
    });
  }
}
