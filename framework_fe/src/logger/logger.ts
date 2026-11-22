import {buildEvent, type EventInput, toError} from "./event";
import {Mask} from "./mask";
import {watchNavigation} from "./navigation";
import {Queue} from "./queue";
import type {LogFields, LoggerOptions} from "./types";
import {watchWindowError} from "./watch";

const DEFAULT_SEND_INTERVAL = 60_000;
const MIN_SEND_INTERVAL = 1_000;

export class Logger {
    private static initialized = false;
    private static queue = new Queue();
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
            if (document.visibilityState === "hidden") Logger.queue.send();
        });
        window.addEventListener("pagehide", () => Logger.queue.send());
        watchWindowError((action, error) => Logger.error(action, error));
        setInterval(() => Logger.queue.send(), sendInterval);
    }

    public static trackNavigation(): void {
        if (Logger.tracking) return;
        Logger.tracking = true;
        watchNavigation(url => Logger.addContext("requestUrl", Logger.mask.url(url)));
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
        Logger.record({action, level: "info", extra});
    }

    public static warn(action: string, error?: Error, extra: LogFields = {}): void {
        Logger.record({action, level: "warn", error, extra});
    }

    public static error(action: string, error: Error, extra: LogFields = {}): void {
        Logger.record({action, level: "error", error, extra});
    }

    public static async capture<T>(actionName: string, work: (extra: LogFields) => Promise<T>): Promise<T> {
        const start = performance.now();
        const extra: LogFields = {};
        const latencyMs = () => Math.round(performance.now() - start);
        try {
            const output = await work(extra);
            Logger.record({action: actionName, level: "info", extra, latencyMs: latencyMs()});
            return output;
        } catch (error) {
            Logger.record({action: actionName, level: "error", error: toError(error), extra, latencyMs: latencyMs()});
            throw error;
        }
    }

    private static record(input: Omit<EventInput, "context">): void {
        Logger.queue.push(buildEvent(Logger.mask, {...input, context: Logger.context}));
    }
}
