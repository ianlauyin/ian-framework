const VERSION_URL = "/version";

export type VersionCheckerOptions = {
  onUpdated: (onReload: () => Promise<void>) => void;
  checkInterval?: number;
};

export class VersionChecker {
  private static currentVersion: string | null = null;
  private static initialized = false;
  private static onUpdated: ((onReload: () => Promise<void>) => void) | undefined;

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
        VersionChecker.onUpdated?.(reload);
      }
    } catch {
      // ignore network errors — version check is best-effort
    }
  }
}

// A plain reload can reuse a cached index.html, which still names the old assets.
async function reload(): Promise<void> {
  try {
    await fetch(location.href, { cache: "reload" });
  } catch {
    // The reload below still runs when the fetch fails.
  }
  location.reload();
}
