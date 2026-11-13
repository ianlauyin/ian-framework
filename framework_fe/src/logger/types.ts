export type LogLevel = "info" | "warn" | "error";
export type LogFields = Record<string, string | number | boolean>;

export type ClientLogEvent = {
  ts: string;
  action: string;
  latency_ms: number | null;
  level: LogLevel;
  error_message: string | null;
  context: LogFields;
  extra: LogFields;
};

export type LogEvent = {
  action: string;
  level: LogLevel;
  errorMessage?: string | null;
  context?: LogFields;
  extra?: LogFields;
  latencyMs?: number;
};
