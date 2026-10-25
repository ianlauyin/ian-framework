import { afterEach, describe, expect, it, vi } from "vitest";

import { AjaxError, ajax, setUnauthorizedHandler } from "../src/ajax";

describe("ajax", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    setUnauthorizedHandler();
  });

  it("sends GET requests with query parameters", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "[]",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/guest", "GET");

    expect(fetchMock).toHaveBeenCalledWith(
      "/ajax/guest",
      expect.objectContaining({ method: "GET", credentials: "same-origin" }),
    );
  });

  it("substitutes path parameters and sends JSON bodies for PUT requests", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 204,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/guest/{id}", "PUT", { name: "Alice" }, { id: "guest-1" });

    expect(fetchMock).toHaveBeenCalledWith(
      "/ajax/guest/guest-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ name: "Alice" }),
        headers: { "Content-Type": "application/json" },
      }),
    );
  });

  it("throws AjaxError when the response is not ok", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        text: async () => JSON.stringify({ message: "Guest not found" }),
      }),
    );

    await expect(
      ajax("/guest/{id}", "DELETE", null, { id: "missing" }),
    ).rejects.toMatchObject({
      name: "Error",
      message: "Guest not found",
      code: 404,
    } satisfies Partial<AjaxError>);
  });

  it("notifies the unauthorized handler and throws AjaxError for 401 responses", async () => {
    const onUnauthorized = vi.fn();
    setUnauthorizedHandler(onUnauthorized);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        text: async () => "",
      }),
    );

    await expect(ajax("/guest", "GET")).rejects.toMatchObject({
      message: "Unknown error",
      code: 401,
    } satisfies Partial<AjaxError>);
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it("parses JSON error bodies with a message field", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 403,
        text: async () =>
          JSON.stringify({ message: "Invalid username or password" }),
      }),
    );

    await expect(
      ajax("/admin/login", "POST", { username: "x", password: "y" }),
    ).rejects.toMatchObject({
      message: "Invalid username or password",
      code: 403,
    } satisfies Partial<AjaxError>);
  });

  it("returns null for 204 responses with an empty body", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 204,
        text: async () => "",
      }),
    );

    await expect(
      ajax("/admin/login", "POST", { username: "admin", password: "secret" }),
    ).resolves.toBeNull();
  });

  it("returns null for 201 and 204 responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 201,
        text: async () => "",
      }),
    );

    await expect(
      ajax("/guest", "POST", {
        name: "Alice",
        relationship: "groom_friend",
        count: 1,
      }),
    ).resolves.toBeNull();
  });
});
