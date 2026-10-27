const HKT_OFFSET_MS = 8 * 60 * 60 * 1000;

export function hktIsoString(date: Date): string {
  return new Date(date.getTime() + HKT_OFFSET_MS).toISOString().replace("Z", "+08:00");
}
