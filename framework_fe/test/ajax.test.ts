import { afterEach, describe, expect, it, vi } from "vitest";

import { type ApiError, ajax, setUnauthorizedHandler } from "../src/ajax";

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

    await ajax("/item", "GET");

    expect(fetchMock).toHaveBeenCalledWith(
      "/ajax/item",
      expect.objectContaining({ method: "GET", credentials: "same-origin" }),
    );
  });

  it("leaves out undefined query values", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "[]",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/item", "GET", { page: 2, filter: undefined });

    expect(fetchMock).toHaveBeenCalledWith("/ajax/item?page=2", expect.anything());
  });

  it("adds no question mark when there is no query value", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "[]",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/item", "GET", {});
    await ajax("/item", "GET", { filter: undefined });

    expect(fetchMock).toHaveBeenNthCalledWith(1, "/ajax/item", expect.anything());
    expect(fetchMock).toHaveBeenNthCalledWith(2, "/ajax/item", expect.anything());
  });

  it("substitutes path parameters and sends JSON bodies for PUT requests", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 204,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/item/{id}", "PUT", { name: "Alice" }, { id: "item-1" });

    expect(fetchMock).toHaveBeenCalledWith(
      "/ajax/item/item-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ name: "Alice" }),
        headers: { "Content-Type": "application/json" },
      }),
    );
  });

  it("puts the path param value 0 in the URL", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/x/{id}", "GET", null, { id: 0 });

    expect(fetchMock).toHaveBeenCalledWith("/ajax/x/0", expect.anything());
  });

  it("encodes path param values", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/x/{id}", "GET", null, { id: "a/b" });

    expect(fetchMock).toHaveBeenCalledWith("/ajax/x/a%2Fb", expect.anything());
  });

  it.each([undefined, null, ""])(
    "throws and sends no request when a path param is %j",
    async (value) => {
      const fetchMock = vi.fn();
      vi.stubGlobal("fetch", fetchMock);

      await expect(ajax("/x/{id}", "GET", null, { id: value })).rejects.toThrow(
        "Path param id is required",
      );
      expect(fetchMock).not.toHaveBeenCalled();
    },
  );

  it("throws ApiError when the response is not ok", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        text: async () => JSON.stringify({ message: "Item not found" }),
      }),
    );

    await expect(
      ajax("/item/{id}", "DELETE", null, { id: "missing" }),
    ).rejects.toMatchObject({
      name: "Error",
      message: "Item not found",
      code: 404,
    } satisfies Partial<ApiError>);
  });

  it("notifies the unauthorized handler and throws ApiError for 401 responses", async () => {
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

    await expect(ajax("/item", "GET")).rejects.toMatchObject({
      message: "Unknown error",
      code: 401,
    } satisfies Partial<ApiError>);
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it("throws ApiError with Unknown error when the error body is not JSON", async () => {
    const onUnauthorized = vi.fn();
    setUnauthorizedHandler(onUnauthorized);
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 502,
      text: async () => "<html><body>Bad Gateway</body></html>",
    });
    await expect(ajax("/item", "GET")).rejects.toMatchObject({
      name: "Error",
      message: "Unknown error",
      code: 502,
    } satisfies Partial<ApiError>);

    fetchMock.mockResolvedValueOnce({
      ok: false,
      status: 401,
      text: async () => "Unauthorized",
    });
    await expect(ajax("/item", "GET")).rejects.toMatchObject({
      message: "Unknown error",
      code: 401,
    } satisfies Partial<ApiError>);
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
      ajax("/login", "POST", { username: "x", password: "y" }),
    ).rejects.toMatchObject({
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
      await expect(ajax("/item", "GET")).rejects.toMatchObject({
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

    await expect(
      ajax("/login", "POST", { username: "admin", password: "secret" }),
    ).resolves.toBeNull();
  });

  it("throws ApiError when a 2xx body is not JSON", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        text: async () => "<html>gateway page</html>",
      }),
    );

    await expect(ajax("/item", "GET")).rejects.toMatchObject({
      message: "Unknown error",
      code: 200,
    } satisfies Partial<ApiError>);
  });

  it("refuses a path param that is a dot segment", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    for (const id of [".", ".."]) {
      await expect(ajax("/item/{id}", "GET", null, { id })).rejects.toThrow("not valid");
    }
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
