import { afterEach, describe, expect, it, vi } from "vitest";

import { createStorage } from "../../src/util/storage";

type Schema = { user: { name: string }; count: number };

function fakeStorage(overrides: Partial<Storage> = {}) {
  const data = new Map<string, string>();
  return {
    data,
    storage: {
      getItem: (key: string) => data.get(key) ?? null,
      setItem: (key: string, value: string) => void data.set(key, value),
      removeItem: (key: string) => void data.delete(key),
      ...overrides,
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("createStorage", () => {
  it("keeps JSON values under the key with the prefix", () => {
    const { data, storage } = fakeStorage();
    vi.stubGlobal("localStorage", storage);
    const store = createStorage<Schema>("app");

    store.set("user", { name: "Alice" });

    expect(data.get("app:user")).toBe('{"name":"Alice"}');
    expect(store.get("user")).toEqual({ name: "Alice" });
    store.remove("user");
    expect(store.get("user")).toBeNull();
  });

  it("uses sessionStorage for the session area", () => {
    const local = fakeStorage();
    const session = fakeStorage();
    vi.stubGlobal("localStorage", local.storage);
    vi.stubGlobal("sessionStorage", session.storage);

    createStorage<Schema>("app", "session").set("count", 1);

    expect(session.data.get("app:count")).toBe("1");
    expect(local.data.size).toBe(0);
  });

  it("gives null for a value that is not JSON", () => {
    const { data, storage } = fakeStorage();
    vi.stubGlobal("localStorage", storage);
    data.set("app:count", "{broken");

    expect(createStorage<Schema>("app").get("count")).toBeNull();
  });

  it("does not throw when the browser blocks the storage", () => {
    const blocked = () => {
      throw new Error("blocked");
    };
    const { storage } = fakeStorage({ getItem: blocked, setItem: blocked, removeItem: blocked });
    vi.stubGlobal("localStorage", storage);
    const store = createStorage<Schema>("app");

    expect(store.get("count")).toBeNull();
    expect(() => store.set("count", 1)).not.toThrow();
    expect(() => store.remove("count")).not.toThrow();
  });
});
