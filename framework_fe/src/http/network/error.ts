import { notifyUnauthorized } from "./hooks";

// A gateway or a proxy answers with these statuses when the app is not reached.
// 503 is not in the list, because the app uses it for a planned stop.
export const NETWORK_ERROR_STATUS_CODES = [0, 502, 504];

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

// A proxy can answer with an HTML or text error page, which must still give an ApiError.
function parseErrorBody(body: string): ErrorBody | null {
  try {
    return body ? JSON.parse(body) : null;
  } catch {
    return null;
  }
}
