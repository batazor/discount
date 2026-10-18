// Timestamps are Unix seconds in the contract. Forms offer a datetime-local input, read as
// UTC (the input carries no zone), and accept a raw integer too; the integer is what is sent.

const INT = /^-?\d+$/;
const LOCAL = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2})?$/;

/** `""` → null, an integer → itself, `YYYY-MM-DDTHH:mm` (UTC) → seconds, anything else → undefined. */
export function parseTimestamp(s: string): number | null | undefined {
  const v = s.trim();
  if (v === "") return null;
  if (INT.test(v)) {
    const n = Number(v);
    return Number.isSafeInteger(n) ? n : undefined;
  }
  if (LOCAL.test(v)) {
    const ms = Date.parse(`${v}Z`);
    return Number.isNaN(ms) ? undefined : Math.floor(ms / 1000);
  }
  return undefined;
}

/** Seconds → `YYYY-MM-DDTHH:mm` in UTC for a datetime-local input; null → `""`. */
export function toDatetimeLocal(ts: number | null): string {
  if (ts === null) return "";
  return new Date(ts * 1000).toISOString().slice(0, 16);
}

/** Seconds → a readable UTC string for display next to the integer. */
export function formatTimestamp(ts: number | null): string {
  if (ts === null) return "—";
  return `${new Date(ts * 1000).toISOString().replace("T", " ").slice(0, 19)} UTC (${ts})`;
}
