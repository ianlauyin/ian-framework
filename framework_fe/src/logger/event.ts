import {hktIsoString} from "../util/time";
import type {Mask} from "./mask";
import type {ClientLogEvent, LogFields, LogLevel} from "./types";

const UNKNOWN_ERROR = "unknown error";
const MAX_STACK_CHARS = 4096;

export interface EventInput {
    action: string;
    level: LogLevel;
    context: LogFields;
    extra: LogFields;
    error?: Error;
    latencyMs?: number;
}

export function buildEvent(mask: Mask, input: EventInput): ClientLogEvent {
    const error = input.error;
    return {
        ts: hktIsoString(new Date()),
        action: input.action,
        latencyMs: input.latencyMs ?? null,
        level: input.level,
        errorMessage: error === undefined ? null : errorMessage(mask, input.level, error),
        errorName: error === undefined ? null : errorName(error),
        stack: error === undefined ? null : stack(mask, error),
        context: {...input.context},
        extra: mask.fields(input.extra),
    };
}

export function toError(value: unknown): Error {
    return value instanceof Error ? value : errorWithoutStack(String(value));
}

export function errorWithoutStack(message: string): Error {
    const error = new Error(message);
    delete error.stack;
    return error;
}

function errorMessage(mask: Mask, level: LogLevel, error: Error): string {
    if (level === "error" && error.message.trim() === "") return UNKNOWN_ERROR;
    return mask.text(error.message);
}

function errorName(error: Error): string | null {
    return error.name === "" || error.name === "Error" ? null : error.name;
}

function stack(mask: Mask, error: Error): string | null {
    if (error.stack === undefined) return null;
    const header = `${error.name}: ${error.message}\n`;
    const frames = error.stack.startsWith(header) ? error.stack.slice(header.length) : error.stack;
    if (frames.trim() === "") return null;
    return mask.text(frames).slice(0, MAX_STACK_CHARS);
}
