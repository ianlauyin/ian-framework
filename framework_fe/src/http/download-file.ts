import {networkError, responseError} from "./network";

const REVOKE_DELAY = 40_000;

export async function downloadFile(url: string, name: string): Promise<void> {
    let response: Response;
    let body: Blob;
    try {
        response = await fetch(url, {credentials: "same-origin"});
        body = await response.blob();
    } catch (error) {
        throw networkError(error, url);
    }
    if (!response.ok) throw responseError(response.status, await body.text(), url);

    const href = URL.createObjectURL(body);
    const link = document.createElement("a");
    link.href = href;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(href), REVOKE_DELAY);
}
