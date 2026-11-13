import type { ClientLogEvent } from "./types";

const LOG_URL = "/log";
const FLUSH_INTERVAL = 60_000;
const MAX_QUEUE = 1000;
const DUPLICATE_ERROR_WINDOW = 5000;

let queue: ClientLogEvent[] = [];
let sending = false;
let started = false;

export function enqueue(event: ClientLogEvent): void {
  start();
  if (isRepeatedError(event)) return;

  if (queue.length >= MAX_QUEUE) queue.shift();
  queue.push(event);
}

// A loop can make the same error many times. The queue keeps one event for the same action and message in 5 seconds.
function isRepeatedError(event: ClientLogEvent): boolean {
  const last = queue[queue.length - 1];
  return (
    event.level === "error" &&
    last !== undefined &&
    last.level === "error" &&
    last.action === event.action &&
    last.error_message === event.error_message &&
    Date.parse(event.ts) - Date.parse(last.ts) <= DUPLICATE_ERROR_WINDOW
  );
}

function start(): void {
  if (started) return;
  started = true;
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") flush(true);
  });
  if (typeof window !== "undefined") window.addEventListener("pagehide", () => beacon());
  setInterval(() => flush(false), FLUSH_INTERVAL);
}

// A beacon is sent even when the page is closing. It cannot be read back, so a failed beacon loses the events.
function beacon(): void {
  if (queue.length === 0) return;
  if (typeof navigator === "undefined" || typeof navigator.sendBeacon !== "function") {
    flush(true);
    return;
  }

  const events = queue;
  queue = [];
  const body = new Blob([JSON.stringify({ events })], { type: "application/json" });
  if (!navigator.sendBeacon(LOG_URL, body)) restore(events);
}

async function flush(keepalive: boolean): Promise<void> {
  if (queue.length === 0 || sending) return;

  const events = queue;
  queue = [];
  sending = true;
  try {
    const response = await fetch(LOG_URL, {
      method: "POST",
      credentials: "same-origin",
      keepalive,
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ events }),
    });
    // The status 400 to 405 fails again with the same events, so the events are dropped. Other failures send again later.
    const isRejected = response.status >= 400 && response.status <= 405;
    if (!response.ok && !isRejected) restore(events);
  } catch {
    restore(events);
  } finally {
    sending = false;
  }
}

function restore(events: ClientLogEvent[]): void {
  queue = [...events, ...queue].slice(-MAX_QUEUE);
}
