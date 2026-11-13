import { hktIsoString } from "../util/time";
import { maskFields, maskText, setMaskPatterns } from "./mask";
import { ActionScope, OUT_OF_SCOPE } from "./scope";
import { enqueue } from "./transport";
import type { ClientLogEvent, LogFields, LogLevel, LogScope } from "./types";

export type { LogFields, LogLevel, LogScope } from "./types";

const UNKNOWN_ERROR = "unknown error";

export class Logger {
  private static scopes: ActionScope[] = [];
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

  // Returns the scope of the running wrapped action. Call it before the first `await` of the action.
  // The scope can be used after an `await`, until the action ends.
  public static scope(): LogScope {
    return Logger.scopes[Logger.scopes.length - 1] ?? OUT_OF_SCOPE;
  }

  public static info(action: string, context: LogFields = {}): void {
    Logger.record("info", action, null, context);
  }

  public static warn(action: string, context: LogFields = {}, error?: unknown): void {
    Logger.record("warn", action, error === undefined ? null : Logger.errorText(error), context);
  }

  public static error(action: string, context: LogFields, error: unknown): void {
    Logger.record("error", action, Logger.errorText(error), context);
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
        Logger.record("info", name, null, {}, { ...scope.close(), ...argsExtra }, performance.now() - start);
      const fail = (error: unknown) => {
        const failure = Logger.failure(error);
        Logger.record(
          "error",
          name,
          failure.error,
          {},
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

  private static errorText(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  private static failure(error: unknown): { error: string } {
    const message = error instanceof Error ? error.message : "";
    return { error: message.trim() === "" ? UNKNOWN_ERROR : message };
  }

  private static record(
    level: LogLevel,
    action: string,
    errorMessage: string | null,
    context: LogFields,
    extra: LogFields = {},
    latencyMs?: number,
  ): void {
    if (action.trim() === "") return;

    const event: ClientLogEvent = {
      ts: hktIsoString(new Date()),
      action,
      latency_ms: latencyMs === undefined ? null : Math.round(latencyMs),
      level,
      error_message: Logger.errorMessage(level, errorMessage),
      context: { request_url: maskText(location.href), ...Logger.context, ...maskFields(context) },
      extra: maskFields({ page: location.pathname, ...extra }),
    };
    enqueue(event);
  }

  private static errorMessage(level: LogLevel, errorMessage: string | null): string | null {
    if (level === "error") {
      const text = errorMessage === null || errorMessage.trim() === "" ? UNKNOWN_ERROR : errorMessage;
      return maskText(text);
    }
    return errorMessage === null ? null : maskText(errorMessage);
  }
}
