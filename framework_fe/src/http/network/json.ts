const ISO_DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/;

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

function reviveDate(_key: string, value: unknown): unknown {
  return typeof value === "string" && ISO_DATE_TIME.test(value) ? new Date(value) : value;
}
