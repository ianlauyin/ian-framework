import { Logger } from "./logger";
import type { LogEvent } from "./types";

// `toEvent` makes the event from the output. The latency is set after it, so it cannot replace the latency.
export async function withLog<T>(work: Promise<T>, toEvent: (output: T) => LogEvent): Promise<T> {
  const start = performance.now();
  const output = await work;
  const event = toEvent(output);
  Logger.record({ ...event, latencyMs: performance.now() - start });
  return output;
}
