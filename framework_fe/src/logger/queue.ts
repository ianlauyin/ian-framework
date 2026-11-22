import type {ClientLogEvent} from "./types";

const LOG_URL = "/log";
const MAX_BEACON_BYTES = 60_000;
const MAX_BUFFERED_EVENTS = 1000;
const BATCH_OVERHEAD_BYTES = '{"events":[]}'.length;

export class Queue {
    private events: ClientLogEvent[] = [];

    public push(event: ClientLogEvent): void {
        this.events.push(event);
        if (this.events.length > MAX_BUFFERED_EVENTS) this.events.shift();
    }

    public send(): void {
        while (this.events.length > 0) {
            const count = this.batchSize();
            if (count === 0) {
                this.events.shift();
                continue;
            }
            const events = this.events.slice(0, count);
            const body = new Blob([JSON.stringify({events})], {type: "application/json"});
            if (!navigator.sendBeacon(LOG_URL, body)) return;
            this.events = this.events.slice(count);
        }
    }

    private batchSize(): number {
        const encoder = new TextEncoder();
        let bytes = BATCH_OVERHEAD_BYTES;
        let count = 0;
        for (const event of this.events) {
            bytes += encoder.encode(JSON.stringify(event)).length + 1;
            if (bytes > MAX_BEACON_BYTES) break;
            count += 1;
        }
        return count;
    }
}
