export type LogLevel = "info" | "warn" | "error";
export type LogFields = Record<string, string | number | boolean>;

export type ClientLogEvent = {
  ts: string;
  action: string;
  latencyMs: number | null;
  level: LogLevel;
  errorMessage: string | null;
  context: LogFields;
  extra: LogFields;
};

export type CaptureEvent = Omit<ClientLogEvent,  "ts"| "latencyMs" | "action" | "context">;
