import { hktIsoString } from "./time";

const LOG_URL = "/log";
const FLUSH_INTERVAL = 60_000;
const MAX_QUEUE = 1000;
const MASK = "***";
const UNKNOWN_ERROR = "unknown error";

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

export class Logger {
  private static queue: ClientLogEvent[] = [];
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
    window.addEventListener("error", (event) => {
      const message = event.message || "window error";
      Logger.error("WINDOW/error", message);
    });
    window.addEventListener("unhandledrejection", (event) => {
      const reason = event.reason;
      const message = reason instanceof Error ? reason.message : "unhandled rejection";
      Logger.error("WINDOW/unhandledrejection", message);
    });
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
      const succeed = () => Logger.record("info", name, null, argsExtra, performance.now() - start);
      const fail = (error: unknown) => {
        const failure = Logger.failure(error);
        Logger.record(
          "error",
          name,
          failure.error,
          { ...argsExtra, ...failure },
          performance.now() - start,
        );
      };

      let result: Result;
      try {
        result = action(...args);
      } catch (error) {
        fail(error);
        throw error;
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

    if (Logger.queue.length >= MAX_QUEUE) Logger.queue.shift();
    Logger.queue.push({
      ts: hktIsoString(new Date()),
      action,
      latency_ms: latencyMs === undefined ? null : Math.round(latencyMs),
      level,
      error_message: Logger.errorMessage(level, errorMessage),
      context: { request_url: Logger.maskText(location.href), ...Logger.context },
      extra: Logger.mask({ page: location.pathname, ...extra }),
    });
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
