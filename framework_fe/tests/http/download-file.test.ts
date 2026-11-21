import {afterEach, describe, expect, it, vi} from "vitest";

import {downloadFile, NetworkConnectionException} from "../../src/http";

function stubDownloadPage() {
    const createObjectURL = vi.fn(() => "blob:file");
    const click = vi.fn();
    const appendChild = vi.fn();
    vi.stubGlobal("URL", {createObjectURL, revokeObjectURL: vi.fn()});
    vi.stubGlobal("document", {
        createElement: () => ({click, remove: vi.fn()}),
        body: {appendChild},
    });
    return {createObjectURL, click, appendChild};
}

describe("downloadFile", () => {
    afterEach(() => {
        vi.unstubAllGlobals();
    });

    it("throws and does not save the body of a 403 or a 404", async () => {
        const page = stubDownloadPage();

        for (const status of [403, 404]) {
            vi.stubGlobal(
                "fetch",
                vi.fn().mockResolvedValue({
                    ok: false,
                    status,
                    blob: async () => new Blob(["denied"]),
                })
            );

            await expect(downloadFile("/file", "report.pdf")).rejects.toBeInstanceOf(NetworkConnectionException);
            await expect(downloadFile("/file", "report.pdf")).rejects.toMatchObject({
                message: `HTTP ${status}`,
                requestURL: "/file",
            });
        }

        expect(page.createObjectURL).not.toHaveBeenCalled();
        expect(page.click).not.toHaveBeenCalled();
        expect(page.appendChild).not.toHaveBeenCalled();
    });
});
