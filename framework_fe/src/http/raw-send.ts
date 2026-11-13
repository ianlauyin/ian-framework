import { ApiError, type ErrorBody, NetworkConnectionException, networkError } from "./network";

// A gateway or a proxy answers with these statuses when the app is not reached.
// 503 is not in the list, because the app uses it for a planned stop.
const NETWORK_ERROR_STATUS_CODES = [0, 502, 504];

const ISO_DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

export type RequestInterceptor = (request: { readonly headers: Headers }) => void | Promise<void>;

let unauthorizedHandler: (() => void) | undefined;
let requestInterceptor: RequestInterceptor | undefined;

export function setUnauthorizedHandler(handler?: () => void) {
  unauthorizedHandler = handler;
}

export function setRequestInterceptor(interceptor?: RequestInterceptor) {
  requestInterceptor = interceptor;
}

// Sends one request with the interceptor, and returns the parsed JSON body, or null for an empty body.
export async function rawSend(url: string, init: RequestInit & { headers: Headers }): Promise<any> {
  await requestInterceptor?.({ headers: init.headers });

  let response: Response;
  let body: string;
  try {
    response = await fetch(url, { ...init, credentials: "same-origin" });
    body = await response.text();
  } catch (error) {
    throw networkError(error, url);
  }

  if (!response.ok) {
    if (response.status === 401) unauthorizedHandler?.();
    if (NETWORK_ERROR_STATUS_CODES.includes(response.status)) {
      throw new NetworkConnectionException(`HTTP ${response.status}`, url);
    }
    throw new ApiError(response.status, parseErrorBody(body), url);
  }
  if (!body) return null;
  try {
    return JSON.parse(body, reviveDate);
  } catch {
    throw new NetworkConnectionException(`response is not JSON, status ${response.status}`, url);
  }
}

// A proxy can answer with an HTML or text error page, which must still give an ApiError.
function parseErrorBody(body: string): ErrorBody | null {
  try {
    return body ? JSON.parse(body) : null;
  } catch {
    return null;
  }
}

function reviveDate(_key: string, value: unknown): unknown {
  return typeof value === "string" && ISO_DATE_TIME.test(value) ? new Date(value) : value;
}
