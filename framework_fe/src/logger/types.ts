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
