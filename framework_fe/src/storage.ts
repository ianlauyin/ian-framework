export interface TypedStorage<Schema extends object> {
  get<K extends keyof Schema & string>(key: K): Schema[K] | null;
  set<K extends keyof Schema & string>(key: K, value: Schema[K]): void;
  remove(key: keyof Schema & string): void;
}

export function createStorage<Schema extends object>(
  prefix: string,
  area: "local" | "session" = "local",
): TypedStorage<Schema> {
  const storage = () => (area === "local" ? localStorage : sessionStorage);
  return {
    get(key) {
      try {
        const text = storage().getItem(`${prefix}:${key}`);
        return text === null ? null : JSON.parse(text);
      } catch {
        return null;
      }
    },
    set(key, value) {
      try {
        storage().setItem(`${prefix}:${key}`, JSON.stringify(value));
      } catch {
        // Blocked or full: the value is not kept
      }
    },
    remove(key) {
      try {
        storage().removeItem(`${prefix}:${key}`);
      } catch {
        // Blocked: nothing to remove
      }
    },
  };
}
