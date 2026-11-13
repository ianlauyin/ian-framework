import type { LogFields } from "./types";

const MASK = "***";

export class Mask {
  private readonly patterns: RegExp[];

  // The `g` flag makes `replace` change each match, not only the first.
  constructor(patterns: RegExp[]) {
    this.patterns = patterns.map(
      (pattern) =>
        new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`),
    );
  }

  public text(text: string): string {
    return this.patterns.reduce((masked, pattern) => masked.replace(pattern, MASK), text);
  }

  public fields(fields: LogFields): LogFields {
    if (this.patterns.length === 0) return fields;

    const masked: LogFields = {};
    for (const [key, value] of Object.entries(fields)) {
      if (this.patterns.some((pattern) => key.search(pattern) !== -1)) {
        masked[key] = MASK;
      } else if (typeof value === "string") {
        masked[key] = this.text(value);
      } else {
        masked[key] = value;
      }
    }
    return masked;
  }
}
