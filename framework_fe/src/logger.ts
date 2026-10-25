const LOG_URL = "/log";
const FLUSH_INTERVAL = 60_000;
const MAX_QUEUE = 1000;

export type LogLevel = "info" | "warn" | "error";
export type LogExtra = Record<string, string | number | boolean>;

type ClientLogEvent = {
  ts: string;
  id: string;
  ip: string;
  user_agent: string;
  action: string;
  status: null;
  latency_ms: null;
  level: LogLevel;
  message: string;
  extra: LogExtra;
};

export class Logger {
  private static queue: ClientLogEvent[] = [];
  private static started = false;

  public static info(message: string, extra?: LogExtra): void {
    Logger.record("info", message, extra);
  }

  public static warn(message: string, extra?: LogExtra): void {
    Logger.record("warn", message, extra);
  }

  public static error(message: string, extra?: LogExtra): void {
    Logger.record("error", message, extra);
  }

  private static record(level: LogLevel, message: string, extra?: LogExtra): void {
    // The route rejects the whole batch when one message is empty.
    if (message.trim() === "") return;
    Logger.start();

    if (Logger.queue.length >= MAX_QUEUE) Logger.queue.shift();
    Logger.queue.push({
      ts: new Date().toISOString(),
      id: "",
      ip: "",
      user_agent: "",
      action: "",
      status: null,
      latency_ms: null,
      level,
      message,
      extra: { page: location.pathname, ...extra },
    });
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
