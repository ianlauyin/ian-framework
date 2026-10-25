const VERSION_URL = "/version";

type VersionUpdatedHandler = () => void;

export class VersionChecker {
  private static currentVersion: string | null = null;
  private static initialized = false;
  private static onUpdated: VersionUpdatedHandler | undefined;

  public static init(): void {
    if (VersionChecker.initialized) return;
    VersionChecker.initialized = true;

    VersionChecker.check();
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") {
        VersionChecker.check();
      }
    });
  }

  public static setUpdatedHandler(handler?: VersionUpdatedHandler): void {
    VersionChecker.onUpdated = handler;
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
