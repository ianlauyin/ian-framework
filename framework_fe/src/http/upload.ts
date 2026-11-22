import {rawSend} from "./raw-send";

export async function upload<Res>(url: string, formField: string, file: File): Promise<Res> {
    const body = new FormData();
    body.append(formField, file);
    return rawSend(url, {method: "POST", headers: new Headers({Accept: "application/json"}), body});
}
