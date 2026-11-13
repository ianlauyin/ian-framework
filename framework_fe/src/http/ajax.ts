import { queryString } from "./network/json";
import { rawSend } from "./network/send";

export { ApiError, type ErrorBody, NetworkConnectionException } from "./network/error";
export { type RequestInterceptor, setRequestInterceptor, setUnauthorizedHandler } from "./network/hooks";

type PathParams = Record<string, any> | null;
type AJAXRequest = Record<string, any> | null;
type AJAXResponse = Record<string, any> | null;

type PathParamValues<Url extends string> = Url extends `${string}{${infer Name}}${infer Rest}`
  ? { [Key in Name | keyof PathParamValues<Rest>]: string | number | boolean }
  : Record<never, never>;

type PathParamsArgument<Url extends string> = keyof PathParamValues<Url> extends never
  ? [pathParams?: PathParams]
  : [pathParams: PathParamValues<Url>];

export const ajax = async <
  Req extends AJAXRequest,
  Res extends AJAXResponse,
  Url extends string = string,
>(
  url: Url,
  method: "GET" | "DELETE" | "POST" | "PUT",
  request?: Req,
  ...[pathParams]: PathParamsArgument<Url>
): Promise<Res> => {
  const isGetOrDelete = method === "GET" || method === "DELETE";

  let parsedUrl =
    "/ajax" +
    url.replace(/{(\w+)}/g, (_, key) => {
      const value = (pathParams as PathParams)?.[key];
      if (value === undefined || value === null || value === "") {
        throw new Error(`Path param ${key} is required`);
      }
      const text = String(value);
      // "." and ".." stay as they are after encoding, and the browser would then change the path.
      if (text === "." || text === "..") {
        throw new Error(`Path param ${key} is not valid`);
      }
      return encodeURIComponent(text);
    });
  if (isGetOrDelete && request) parsedUrl += queryString(request);

  const headers = new Headers();
  if (!isGetOrDelete) headers.set("Content-Type", "application/json");
  return rawSend(parsedUrl, {
    method,
    headers,
    body: isGetOrDelete ? undefined : JSON.stringify(request),
  });
};
