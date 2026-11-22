import {afterEach, describe, expect, it, vi} from "vitest";

import {ApiError, downloadFile, NetworkConnectionException} from "../../src/http";

function stubDownloadPage() {
    const createObjectURL = vi.fn(() => "blob:file");
    const revokeObjectURL = vi.fn();
    const link = {href: "", download: "", click: vi.fn(), remove: vi.fn()};
    const appendChild = vi.fn();
    vi.stubGlobal("URL", {createObjectURL, revokeObjectURL});
    vi.stubGlobal("document", {
        createElement: () => link,
        body: {appendChild},
    });
    return {createObjectURL, revokeObjectURL, link, appendChild};
}

function respondWith(status: number, body: string): void {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ok: status >= 200 && status < 300, status, blob: async () => new Blob([body])}));
}

describe("downloadFile", () => {
    afterEach(() => {
        vi.useRealTimers();
        vi.unstubAllGlobals();
    });

    it("saves the body with the name and revokes the object URL after the delay", async () => {
        vi.useFakeTimers();
        const page = stubDownloadPage();
        respondWith(200, "content");

        await downloadFile("/file", "report.pdf");

        expect(page.link).toMatchObject({href: "blob:file", download: "report.pdf"});
        expect(page.appendChild).toHaveBeenCalledWith(page.link);
        expect(page.link.click).toHaveBeenCalledOnce();
        expect(page.link.remove).toHaveBeenCalledOnce();
        expect(page.revokeObjectURL).not.toHaveBeenCalled();

        vi.advanceTimersByTime(40_000);
        expect(page.revokeObjectURL).toHaveBeenCalledWith("blob:file");
    });

    it("throws ApiError and does not save the body of a 403 or a 404", async () => {
        const page = stubDownloadPage();

        for (const status of [403, 404]) {
            respondWith(status, JSON.stringify({message: "denied"}));

            await expect(downloadFile("/file", "report.pdf")).rejects.toBeInstanceOf(ApiError);
            await expect(downloadFile("/file", "report.pdf")).rejects.toMatchObject({
                message: "denied",
                code: status,
                requestURL: "/file",
            });
        }

        expect(page.createObjectURL).not.toHaveBeenCalled();
        expect(page.link.click).not.toHaveBeenCalled();
        expect(page.appendChild).not.toHaveBeenCalled();
    });

    it("throws NetworkConnectionException for a 502 and when fetch fails", async () => {
        stubDownloadPage();

        respondWith(502, "");
        await expect(downloadFile("/file", "report.pdf")).rejects.toMatchObject({message: "HTTP 502", requestURL: "/file"});
        await expect(downloadFile("/file", "report.pdf")).rejects.toBeInstanceOf(NetworkConnectionException);

        vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
        await expect(downloadFile("/file", "report.pdf")).rejects.toBeInstanceOf(NetworkConnectionException);
    });
});
