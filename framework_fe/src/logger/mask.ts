import type { LogFields } from "./types";

const MASK = "***";

let masks: RegExp[] = [];

export function setMaskPatterns(patterns: RegExp[]): void {
  masks = patterns.map(
    (pattern) =>
      new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`),
  );
}

export function maskText(text: string): string {
  return masks.reduce((masked, mask) => masked.replace(mask, MASK), text);
}

export function maskFields(extra: LogFields): LogFields {
  if (masks.length === 0) return extra;

  const masked: LogFields = {};
  for (const [key, value] of Object.entries(extra)) {
    if (masks.some((mask) => key.search(mask) !== -1)) {
      masked[key] = MASK;
    } else if (typeof value === "string") {
      masked[key] = maskText(value);
    } else {
      masked[key] = value;
    }
  }
  return masked;
}
