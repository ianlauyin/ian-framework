import { NetworkConnectionException, networkError, responseError } from "./error";
import { interceptRequest } from "./hooks";
import { parseJSON } from "./json";

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
