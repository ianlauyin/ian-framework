import { rawSend } from "./network";

// url is used as it is, with no prefix. The result is the parsed JSON body.
export async function upload<Response>(url: string, formField: string, file: File): Promise<Response> {
  const body = new FormData();
  body.append(formField, file);
  return rawSend(url, { method: "POST", headers: new Headers({ Accept: "application/json" }), body });
}
