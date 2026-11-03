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

function sendsJsonBody(method: "GET" | "DELETE" | "POST" | "PUT") {
  return method === "POST" || method === "PUT";
}

function requestUrl(url: string, pathParams: PathParams | undefined, request: AJAXRequest | undefined) {
  const parsedUrl =
    "/ajax" +
    url.replace(/{(\w+)}/g, (_, key: string) => {
      const value = pathParams?.[key];
      if (value === undefined || value === null || value === "") {
        throw new Error(`Path param ${key} is required`);
      }
      return encodeURIComponent(String(value));
    });
  if (request == null) return parsedUrl;
  return withQuery(parsedUrl, request);
}

function withQuery(url: string, request: Record<string, any>) {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(request)) {
    if (value !== undefined) query.append(key, String(value));
  }
  const queryText = query.toString();
  if (queryText !== "") return `${url}?${queryText}`;
  return url;
}

async function readResponse<Res extends AJAXResponse>(response: Response): Promise<Res> {
  const body = await response.text();
  if (response.status === 401) unauthorizedHandler?.();
  if (!response.ok) throw new AjaxError(response.status, parseErrorBody(body));
  if (!body) return null as Res;
  try {
    return JSON.parse(body);
  } catch {
    throw new AjaxError(response.status, null);
  }
}

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
  const sendsBody = sendsJsonBody(method);
  const response = await fetch(requestUrl(url, pathParams, sendsBody ? undefined : request), {
    method,
    credentials: "same-origin",
    body: sendsBody ? JSON.stringify(request) : undefined,
    headers: sendsBody ? { "Content-Type": "application/json" } : undefined,
  });
  return readResponse(response);
};

// A proxy can answer with an HTML or text error page, which must still give an AjaxError.
function parseErrorBody(body: string): ErrorBody | null {
  try {
    return body ? JSON.parse(body) : null;
  } catch {
    return null;
  }
}
