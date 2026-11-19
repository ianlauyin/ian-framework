import { afterEach, describe, expect, it, vi } from "vitest";

import { type ApiError, NetworkConnectionException } from "../../src/http/network";
import { rawSend } from "../../src/http/raw-send";

describe("rawSend", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("throws ApiError when the response is not ok", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        text: async () => JSON.stringify({ message: "Item not found" }),
      }),
    );

    await expect(rawSend("/item/missing", { method: "DELETE" })).rejects.toMatchObject({
      name: "Error",
      message: "Item not found",
      code: 404,
    } satisfies Partial<ApiError>);
  });

  it("throws ApiError for 401 responses", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        text: async () => "",
      }),
    );

    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      message: "Unknown error",
      code: 401,
    } satisfies Partial<ApiError>);
  });

  it("throws ApiError with Unknown error when the error body is not JSON", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: async () => "<html><body>Server error</body></html>",
    });
    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      name: "Error",
      message: "Unknown error",
      code: 500,
    } satisfies Partial<ApiError>);

    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => "Unauthorized",
    });
    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      message: "Unknown error",
      code: 401,
    } satisfies Partial<ApiError>);
  });

  it("parses JSON error bodies with a message field", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 403,
        text: async () => JSON.stringify({ message: "Invalid username or password" }),
      }),
    );

    await expect(rawSend("/login", { method: "POST" })).rejects.toMatchObject({
      message: "Invalid username or password",
      code: 403,
    } satisfies Partial<ApiError>);
  });

  it("uses Unknown error when the body has no string message", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    for (const body of [{}, { message: 7 }]) {
      fetchMock.mockResolvedValueOnce({
        ok: false,
        status: 400,
        text: async () => JSON.stringify(body),
      });
      await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
        message: "Unknown error",
        code: 400,
      } satisfies Partial<ApiError>);
    }
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

    await expect(rawSend("/login", { method: "POST" })).resolves.toBeNull();
  });

  it("throws NetworkConnectionException when a 2xx body is not JSON", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => "<html>gateway page</html>",
      }),
    );

    await expect(rawSend("/item", { method: "GET" })).rejects.toBeInstanceOf(
      NetworkConnectionException,
    );
  });

  it.each([0, 502, 504])("throws NetworkConnectionException for the status %i", async (status) => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status, text: async () => "" }),
    );

    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      message: `HTTP ${status}`,
      requestURL: "/item",
    });
  });

  it("keeps ApiError for the status 503", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 503,
        text: async () => JSON.stringify({ message: "maintenance" }),
      }),
    );

    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      code: 503,
      message: "maintenance",
    });
  });

  it("throws NetworkConnectionException when fetch fails", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));

    await expect(rawSend("/item", { method: "GET" })).rejects.toMatchObject({
      message: "Failed to fetch",
      requestURL: "/item",
    });
  });

  it("turns an ISO date text in the response into a Date", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () =>
          JSON.stringify({
            at: "2026-10-25T20:00:00+08:00",
            day: "2026-10-25",
            nested: [{ at: "2026-10-25T12:00:00.123Z" }],
          }),
      }),
    );

    const result: any = await rawSend("/item", { method: "GET" });

    expect(result.at).toEqual(new Date("2026-10-25T12:00:00Z"));
    expect(result.day).toBe("2026-10-25");
    expect(result.nested[0].at).toEqual(new Date("2026-10-25T12:00:00.123Z"));
  });
});
