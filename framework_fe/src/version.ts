const VERSION_URL = "/version";

export type VersionCheckerOptions = {
    onUpdated: () => void;
    checkInterval?: number;
};

export class VersionChecker {
    private static currentVersion: string | null = null;
    private static initialized = false;
    private static onUpdated: (() => void) | undefined;

    public static init(options: VersionCheckerOptions): void {
        if (VersionChecker.initialized) throw new Error("VersionChecker.init() is called twice");
        VersionChecker.initialized = true;
        VersionChecker.onUpdated = options.onUpdated;

        VersionChecker.check();
        document.addEventListener("visibilitychange", () => {
            if (document.visibilityState === "visible") {
                VersionChecker.check();
            }
        });
        if (options.checkInterval !== undefined) {
            setInterval(() => VersionChecker.check(), options.checkInterval);
        }
    }

    public static async reload(): Promise<void> {
        await fetch(location.href, {cache: "reload"}).catch(() => undefined);
        location.reload();
    }

    private static async check(): Promise<void> {
        try {
            const response = await fetch(VERSION_URL, {
                method: "GET",
                credentials: "same-origin",
                cache: "no-store",
            });
            if (!response.ok) return;

            const body: {version?: unknown} | null = await response.json();
            const version = body?.version;
            if (typeof version !== "string") return;
            if (VersionChecker.currentVersion === null) {
                VersionChecker.currentVersion = version;
                return;
            }
            if (version === VersionChecker.currentVersion) return;

            VersionChecker.onUpdated?.();
        } catch {
            return;
        }
    }
}
