import {afterEach, describe, expect, it, vi} from "vitest";

import {upload} from "../../src/http";

describe("upload", () => {
    afterEach(() => {
        vi.unstubAllGlobals();
    });

    it("posts the file in the form field to the URL and gives the response", async () => {
        const fetchMock = vi.fn().mockResolvedValue({ok: true, status: 200, text: async () => JSON.stringify({id: "file-1"})});
        vi.stubGlobal("fetch", fetchMock);
        const file = new File(["content"], "report.pdf");

        const response = await upload<{id: string}>("/api/file", "attachment", file);

        expect(response).toEqual({id: "file-1"});
        const [url, init] = fetchMock.mock.calls[0];
        expect(url).toBe("/api/file");
        expect(init).toMatchObject({method: "POST", credentials: "same-origin"});
        expect(init.headers.get("Accept")).toBe("application/json");
        expect(init.headers.has("Content-Type")).toBe(false);
        expect(init.body.get("attachment").name).toBe("report.pdf");
    });
});
