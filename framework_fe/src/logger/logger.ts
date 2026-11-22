import {hktIsoString} from "../util/time";
import {Mask} from "./mask";
import type {ClientLogEvent, LogFields, LoggerOptions} from "./types";

const LOG_URL = "/log";
const DEFAULT_SEND_INTERVAL = 60_000;
const MIN_SEND_INTERVAL = 1_000;
const UNKNOWN_ERROR = "unknown error";
const MAX_BEACON_BYTES = 60_000;
const MAX_BUFFERED_EVENTS = 1000;
const BATCH_OVERHEAD_BYTES = '{"events":[]}'.length;

export class Logger {
    private static initialized = false;
    private static buffer: ClientLogEvent[] = [];
    private static mask = new Mask([]);
    private static context: LogFields = {};
    private static tracking = false;

    public static init(options: LoggerOptions): void {
        if (Logger.initialized) throw new Error("Logger.init() is called twice");
        const sendInterval = options.sendInterval ?? DEFAULT_SEND_INTERVAL;
        if (!(sendInterval >= MIN_SEND_INTERVAL)) throw new Error(`Logger sendInterval must be ${MIN_SEND_INTERVAL} or more`);
        Logger.initialized = true;
        Logger.mask = new Mask(options.maskPatterns);
        document.addEventListener("visibilitychange", () => {
            if (document.visibilityState === "hidden") Logger.send();
        });
        window.addEventListener("pagehide", () => Logger.send());
        Logger.watchWindowError();
        setInterval(() => Logger.send(), sendInterval);
    }

    public static trackNavigation(): void {
        if (Logger.tracking) return;
        Logger.tracking = true;
        Logger.navigated();
        const history = window.history;
        const pushState = history.pushState.bind(history);
        const replaceState = history.replaceState.bind(history);
        history.pushState = (...args) => {
            pushState(...args);
            Logger.navigated();
        };
        history.replaceState = (...args) => {
            replaceState(...args);
            Logger.navigated();
        };
        window.addEventListener("popstate", () => Logger.navigated());
    }

    public static addContext(key: string, value: string | number | boolean): void {
        Logger.context = {...Logger.context, [key]: value};
    }

    public static removeContext(...keys: string[]): void {
        const kept = {...Logger.context};
        for (const key of keys) delete kept[key];
        Logger.context = kept;
    }

    public static info(action: string, extra: LogFields = {}): void {
        Logger.record({
            ts: hktIsoString(new Date()),
            action,
            latencyMs: null,
            level: "info",
            errorMessage: null,
            context: {...Logger.context},
            extra: Logger.mask.fields(extra),
        });
    }

    public static warn(action: string, error?: Error, extra: LogFields = {}): void {
        Logger.record({
            ts: hktIsoString(new Date()),
            action,
            latencyMs: null,
            level: "warn",
            errorMessage: error === undefined ? null : Logger.mask.text(error.message),
            context: {...Logger.context},
            extra: Logger.mask.fields(extra),
        });
    }

    public static error(action: string, error: Error, extra: LogFields = {}): void {
        Logger.record({
            ts: hktIsoString(new Date()),
            action,
            latencyMs: null,
            level: "error",
            errorMessage: Logger.errorMessage(error),
            context: {...Logger.context},
            extra: Logger.mask.fields(extra),
        });
    }

    public static async capture<T>(actionName: string, work: (extra: LogFields) => Promise<T>): Promise<T> {
        const start = performance.now();
        const extra: LogFields = {};
        try {
            const output = await work(extra);
            Logger.record({
                ts: hktIsoString(new Date()),
                action: actionName,
                latencyMs: Math.round(performance.now() - start),
                level: "info",
                errorMessage: null,
                context: {...Logger.context},
                extra: Logger.mask.fields(extra),
            });
            return output;
        } catch (error) {
            Logger.record({
                ts: hktIsoString(new Date()),
                action: actionName,
                latencyMs: Math.round(performance.now() - start),
                level: "error",
                errorMessage: Logger.errorMessage(toError(error)),
                context: {...Logger.context},
                extra: Logger.mask.fields(extra),
            });
            throw error;
        }
    }

    private static errorMessage(error: Error): string {
        return Logger.mask.text(error.message.trim() === "" ? UNKNOWN_ERROR : error.message);
    }

    private static record(event: ClientLogEvent): void {
        Logger.buffer.push(event);
        if (Logger.buffer.length > MAX_BUFFERED_EVENTS) Logger.buffer.shift();
    }

    private static navigated(): void {
        Logger.addContext("requestUrl", Logger.mask.url(window.location.href));
    }

    private static watchWindowError(): void {
        window.addEventListener(
            "error",
            event => {
                if (event.error instanceof Error) {
                    Logger.error("WINDOW/error", event.error);
                    return;
                }
                const message = event.message || Logger.domSourceMessage(event.target) || "window error";
                Logger.error("WINDOW/error", new Error(message));
            },
            true
        );
        window.addEventListener("unhandledrejection", event => {
            Logger.error("WINDOW/unhandledrejection", toError(event.reason));
        });
    }

    private static domSourceMessage(target: EventTarget | null): string | null {
        if (typeof Element === "undefined" || !(target instanceof Element)) return null;
        const source = target.getAttribute("src") ?? target.getAttribute("href") ?? "";
        return `DOM source error: <${target.tagName.toLowerCase()}> ${source}`.trim();
    }

    private static send(): void {
        while (Logger.buffer.length > 0) {
            const count = Logger.batchSize();
            if (count === 0) {
                Logger.buffer.shift();
                continue;
            }
            const events = Logger.buffer.slice(0, count);
            const body = new Blob([JSON.stringify({events})], {type: "application/json"});
            if (!navigator.sendBeacon(LOG_URL, body)) return;
            Logger.buffer = Logger.buffer.slice(count);
        }
    }

    private static batchSize(): number {
        const encoder = new TextEncoder();
        let bytes = BATCH_OVERHEAD_BYTES;
        let count = 0;
        for (const event of Logger.buffer) {
            bytes += encoder.encode(JSON.stringify(event)).length + 1;
            if (bytes > MAX_BEACON_BYTES) break;
            count += 1;
        }
        return count;
    }
}

function toError(value: unknown): Error {
    return value instanceof Error ? value : new Error(String(value));
}
