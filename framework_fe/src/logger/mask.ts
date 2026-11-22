import type {LogFields} from "./types";

const MASK = "***";

export class Mask {
    private readonly patterns: RegExp[];

    constructor(patterns: RegExp[]) {
        this.patterns = patterns.map(pattern => new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`));
    }

    public text(text: string): string {
        return this.patterns.reduce((masked, pattern) => masked.replace(pattern, MASK), text);
    }

    public url(url: string): string {
        const start = url.search(/[?#]/);
        if (start === -1) return this.text(url);
        const params = url.slice(start).replace(/([?&#])([^=&#]*)=([^&#]*)/g, (_, separator: string, key: string, value: string) => {
            if (this.matches(key)) return `${separator}${key}=${MASK}`;
            return `${separator}${this.text(key)}=${this.text(value)}`;
        });
        return `${this.text(url.slice(0, start))}${params}`;
    }

    public fields(fields: LogFields): LogFields {
        if (this.patterns.length === 0) return {...fields};

        const masked: LogFields = {};
        for (const [key, value] of Object.entries(fields)) {
            if (this.matches(key)) {
                masked[key] = MASK;
            } else if (typeof value === "string") {
                masked[key] = this.text(value);
            } else {
                masked[key] = value;
            }
        }
        return masked;
    }

    private matches(key: string): boolean {
        return this.patterns.some(pattern => key.search(pattern) !== -1);
    }
}
