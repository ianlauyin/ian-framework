export interface ErrorBody {
  message: string;
}

export class ApiError extends Error {
  public readonly code: number;
  public readonly requestURL: string;

  constructor(code: number, errorBody: ErrorBody | null, requestURL = "") {
    super(typeof errorBody?.message === "string" ? errorBody.message : "Unknown error");
    this.code = code;
    this.requestURL = requestURL;
  }
}

// The request did not give a usable response: the connection failed, a gateway answered, or the body is not JSON.
export class NetworkConnectionException extends Error {
  public readonly requestURL: string;

  constructor(message: string, requestURL: string) {
    super(message);
    this.requestURL = requestURL;
  }
}

export type RequestInterceptor = (request: { readonly headers: Headers }) => void | Promise<void>;

type UnauthorizedHandler = () => void;

// A gateway or a proxy answers with these statuses when the app is not reached.
// 503 is not in the list, because the app uses it for a planned stop.
export const NETWORK_ERROR_STATUS_CODES = [0, 502, 504];
const ISO_DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

let unauthorizedHandler: UnauthorizedHandler | undefined;
let requestInterceptor: RequestInterceptor | undefined;

export function setUnauthorizedHandler(handler?: UnauthorizedHandler) {
  unauthorizedHandler = handler;
}

export function setRequestInterceptor(interceptor?: RequestInterceptor) {
  requestInterceptor = interceptor;
}

export async function interceptRequest(headers: Headers): Promise<void> {
  await requestInterceptor?.({ headers });
}

export function notifyUnauthorized(): void {
  unauthorizedHandler?.();
}

export function networkError(error: unknown, requestURL: string): NetworkConnectionException {
  return new NetworkConnectionException(error instanceof Error ? error.message : String(error), requestURL);
}

// The error for a response that is not ok: a gateway status gives NetworkConnectionException, else ApiError.
export function responseError(status: number, body: string, requestURL: string): ApiError | NetworkConnectionException {
  if (status === 401) notifyUnauthorized();
  if (NETWORK_ERROR_STATUS_CODES.includes(status)) {
    return new NetworkConnectionException(`HTTP ${status}`, requestURL);
  }
  return new ApiError(status, parseErrorBody(body), requestURL);
}

export function queryString(request: object): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(request)) {
    if (value === undefined) continue;
    query.append(key, value instanceof Date ? value.toISOString() : String(value));
  }
  const text = query.toString();
  return text === "" ? "" : `?${text}`;
}

export function parseJSON(text: string): any {
  return JSON.parse(text, reviveDate);
}

// Sends one request with the interceptor, and returns the parsed JSON body, or null for an empty body.
export async function rawSend(url: string, init: RequestInit & { headers: Headers }): Promise<any> {
  await interceptRequest(init.headers);

  let response: Response;
  let body: string;
  try {
    response = await fetch(url, { ...init, credentials: "same-origin" });
    body = await response.text();
  } catch (error) {
    throw networkError(error, url);
  }

  if (!response.ok) throw responseError(response.status, body, url);
  if (!body) return null;
  try {
    return parseJSON(body);
  } catch {
    throw new NetworkConnectionException(`response is not JSON, status ${response.status}`, url);
  }
}

function reviveDate(_key: string, value: unknown): unknown {
  return typeof value === "string" && ISO_DATE_TIME.test(value) ? new Date(value) : value;
}

// A proxy can answer with an HTML or text error page, which must still give an ApiError.
function parseErrorBody(body: string): ErrorBody | null {
  try {
    return body ? JSON.parse(body) : null;
  } catch {
    return null;
  }
}
