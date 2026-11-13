import type { ClientLogEvent } from "./types";

const LOG_URL = "/log";

export class HttpLogShipper {
  private readonly url: string;

  constructor(url: string = LOG_URL) {
    this.url = url;
  }

  // Gives `true` when the events must not be sent again. The status 400 to 405 fails again with the same events, so the events are dropped.
  public async ship(events: ClientLogEvent[], keepalive: boolean): Promise<boolean> {
    try {
      const response = await fetch(this.url, {
        method: "POST",
        credentials: "same-origin",
        keepalive,
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ events }),
      });
      const isRejected = response.status >= 400 && response.status <= 405;
      return response.ok || isRejected;
    } catch {
      return false;
    }
  }

  // A beacon is sent even when the page is closing. It cannot be read back, so a failed beacon loses the events.
  public beacon(events: ClientLogEvent[]): boolean {
    const body = new Blob([JSON.stringify({ events })], { type: "application/json" });
    return navigator.sendBeacon(this.url, body);
  }

  public canBeacon(): boolean {
    return typeof navigator !== "undefined" && typeof navigator.sendBeacon === "function";
  }
}
