import { afterEach, describe, expect, it, vi } from "vitest";

import { ajax } from "../../src/http";

describe("ajax", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
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
      "/api/item",
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

    expect(fetchMock).toHaveBeenCalledWith("/api/item?page=2", expect.anything());
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

    expect(fetchMock).toHaveBeenNthCalledWith(1, "/api/item", expect.anything());
    expect(fetchMock).toHaveBeenNthCalledWith(2, "/api/item", expect.anything());
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
      "/api/item/item-1",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ name: "Alice" }),
      }),
    );
    expect(fetchMock.mock.calls[0][1].headers.get("Content-Type")).toBe("application/json");
  });

  it("sends a JSON body for PATCH requests", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 204,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/item", "PATCH", { name: "Alice" });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/item",
      expect.objectContaining({ method: "PATCH", body: JSON.stringify({ name: "Alice" }) }),
    );
    expect(fetchMock.mock.calls[0][1].headers.get("Content-Type")).toBe("application/json");
  });

  it("puts the path param value 0 in the URL", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/x/{id}", "GET", null, { id: 0 });

    expect(fetchMock).toHaveBeenCalledWith("/api/x/0", expect.anything());
  });

  it("encodes path param values", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      text: async () => "",
    });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/x/{id}", "GET", null, { id: "a/b" });

    expect(fetchMock).toHaveBeenCalledWith("/api/x/a%2Fb", expect.anything());
  });

  it.each([undefined, null, ""])(
    "throws and sends no request when a path param is %j",
    async (value) => {
      const fetchMock = vi.fn();
      vi.stubGlobal("fetch", fetchMock);

      await expect(ajax("/x/{id}", "GET", null, { id: value } as any)).rejects.toThrow(
        "Path param id is required",
      );
      expect(fetchMock).not.toHaveBeenCalled();
    },
  );

  it("sends a Date in the query as an ISO text", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, status: 204, text: async () => "" });
    vi.stubGlobal("fetch", fetchMock);

    await ajax("/item", "GET", { since: new Date("2026-10-25T12:00:00Z") });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/item?since=2026-10-25T12%3A00%3A00.000Z",
      expect.anything(),
    );
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
