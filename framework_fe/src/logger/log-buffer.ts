import type { ClientLogEvent } from "./types";

const CAPACITY = 1000;
const DUPLICATE_ERROR_WINDOW = 5000;

export class LogBuffer {
  private events: ClientLogEvent[] = [];

  public len(): number {
    return this.events.length;
  }

  public push(event: ClientLogEvent): void {
    if (this.isRepeatedError(event)) return;

    if (this.events.length >= CAPACITY) this.events.shift();
    this.events.push(event);
  }

  public take(): ClientLogEvent[] {
    const events = this.events;
    this.events = [];
    return events;
  }

  public restore(events: ClientLogEvent[]): void {
    this.events = [...events, ...this.events].slice(-CAPACITY);
  }

  // A loop can make the same error many times. The buffer keeps one event for the same action and message in 5 seconds.
  private isRepeatedError(event: ClientLogEvent): boolean {
    const last = this.events[this.events.length - 1];
    return (
      event.level === "error" &&
      last !== undefined &&
      last.level === "error" &&
      last.action === event.action &&
      last.error_message === event.error_message &&
      Date.parse(event.ts) - Date.parse(last.ts) <= DUPLICATE_ERROR_WINDOW
    );
  }
}
