import { rawSend } from "./raw-send";

export async function upload<Response>(url: string, formField: string, file: File): Promise<Response> {
  const body = new FormData();
  body.append(formField, file);
  return rawSend(url, { method: "POST", headers: new Headers({ Accept: "application/json" }), body });
}
