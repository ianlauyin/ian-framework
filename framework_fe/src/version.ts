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
    if (VersionChecker.initialized) return;
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

  private static async check(): Promise<void> {
    try {
      const response = await fetch(VERSION_URL, {
        method: "GET",
        credentials: "same-origin",
      });
      if (!response.ok) return;

      const body: { version: string } = await response.json();
      if (VersionChecker.currentVersion === null) {
        VersionChecker.currentVersion = body.version;
        return;
      }

      if (VersionChecker.currentVersion !== body.version) {
        VersionChecker.onUpdated?.();
      }
    } catch {
      // ignore network errors — version check is best-effort
    }
  }
}
