export interface ErrorBody {
  message: string;
  code?: string;
}

export class AjaxError extends Error {
  public readonly code: number;
  public readonly errorCode: string | undefined;

  constructor(code: number, errorBody: ErrorBody | null) {
    super(errorBody?.message ?? "Unknown error");
    this.code = code;
    this.errorCode = typeof errorBody?.code === "string" ? errorBody.code : undefined;
  }
}

type UnauthorizedHandler = () => void;

let unauthorizedHandler: UnauthorizedHandler | undefined;

export function setUnauthorizedHandler(handler?: UnauthorizedHandler) {
  unauthorizedHandler = handler;
}

type PathParams = Record<string, any> | null;
type AJAXRequest = Record<string, any> | null;
type AJAXResponse = Record<string, any> | null;

export const ajax = async <
  Req extends AJAXRequest,
  Res extends AJAXResponse,
  P extends PathParams,
>(
  url: string,
  method: "GET" | "DELETE" | "POST" | "PUT",
  request?: Req,
  pathParams?: P,
): Promise<Res> => {
  const isGetOrDelete = method === "GET" || method === "DELETE";

  let parsedUrl =
    "/ajax" +
    url.replace(/{(\w+)}/g, (_, key) => {
      const value = pathParams?.[key];
      if (value === undefined || value === null || value === "") {
        throw new Error(`Path param ${key} is required`);
      }
      return encodeURIComponent(String(value));
    });
  if (isGetOrDelete && request) {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(request)) {
      if (value !== undefined) query.append(key, String(value));
    }
    const queryText = query.toString();
    if (queryText !== "") parsedUrl = `${parsedUrl}?${queryText}`;
  }

  const response: Response = await fetch(parsedUrl, {
    method,
    credentials: "same-origin",
    body: isGetOrDelete ? undefined : JSON.stringify(request),
    headers: isGetOrDelete ? undefined : { "Content-Type": "application/json" },
  });

  const body = await response.text();

  if (response.status === 401) {
    unauthorizedHandler?.();
  }

  if (!response.ok) {
    throw new AjaxError(response.status, parseErrorBody(body));
  }

  if (!body) return null as Res;
  try {
    return JSON.parse(body);
  } catch {
    throw new AjaxError(response.status, null);
  }
};

// A proxy can answer with an HTML or text error page, which must still give an AjaxError.
function parseErrorBody(body: string): ErrorBody | null {
  try {
    return body ? JSON.parse(body) : null;
  } catch {
    return null;
  }
}
