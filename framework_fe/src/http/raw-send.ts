import {NetworkConnectionException, networkError, responseError} from "./network";

const ISO_DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

export async function rawSend(url: string, init: RequestInit): Promise<any> {
    let response: Response;
    let body: string;
    try {
        response = await fetch(url, {...init, credentials: "same-origin"});
        body = await response.text();
    } catch (error) {
        throw networkError(error, url);
    }

    if (!response.ok) throw responseError(response.status, body, url);
    if (!body) return null;
    try {
        return JSON.parse(body, reviveDate);
    } catch {
        throw new NetworkConnectionException(`response is not JSON, status ${response.status}`, url);
    }
}

function reviveDate(_key: string, value: unknown): unknown {
    return typeof value === "string" && ISO_DATE_TIME.test(value) ? new Date(value) : value;
}
