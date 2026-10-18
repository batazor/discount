// The campaign as one sentence (design.md, decision 2), from form fields, for the review
// step on the server and the live card in the browser.
import type { Fields } from "./fields";
import { parseTimestamp } from "./time";
import { formatAmountInput, parseAmount, parsePercent } from "./units";

const on = (f: Fields, key: string) => ["on", "true", "1"].includes((f[key] ?? "").trim().toLowerCase());

/** `1000.00 EUR` → `1 000.00 EUR` */
export function grouped(minor: number, currency: string): string {
  const [whole, frac] = formatAmountInput(minor, currency).split(".");
  const w = whole!.replace(/\B(?=(\d{3})+(?!\d))/g, " ");
  return `${frac !== undefined ? `${w}.${frac}` : w} ${currency}`;
}

function money(f: Fields, prefix: string): string | null {
  const amount = (f[`${prefix}.amount`] ?? "").trim();
  const currency = (f[`${prefix}.currency`] ?? "").trim();
  if (!amount || !currency) return null;
  const minor = parseAmount(amount, currency);
  return minor === undefined ? `${amount} ${currency}` : grouped(minor, currency);
}

export const utcDate = (ts: number): string =>
  new Date(ts * 1000).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });

function date(f: Fields, key: string, fmt: (ts: number) => string): string | null {
  const t = parseTimestamp(f[key] ?? "");
  return t === null || t === undefined ? null : fmt(t);
}

export function summarize(f: Fields, fmtDate: (ts: number) => string = utcDate): string {
  const parts: string[] = [];
  // what it gives
  if ((f["action.type"] ?? "OrderPercentage") === "OrderPercentage") {
    const bps = parsePercent(f["action.percent"] ?? "");
    parts.push(bps === undefined ? "… % off the order" : `${f["action.percent"]!.trim()} % off the order`);
  } else {
    parts.push(`${money(f, "action.amount") ?? "… off the order"}${money(f, "action.amount") ? " off the order" : ""}`);
  }
  // conditions
  if (on(f, "b.first_order")) parts.push("for first orders only");
  if (on(f, "b.once")) parts.push("once per customer");
  if (on(f, "b.needs_code")) parts.push("with a code");
  if (on(f, "b.allowlist")) parts.push("for allowlisted customers only");
  if (on(f, "b.min_total")) parts.push(`for orders from ${money(f, "b.min_total") ?? "…"}`);
  if (on(f, "b.tags") && (f["b.tags_any"] ?? "").trim()) parts.push(`for carts tagged ${f["b.tags_any"]!.trim()}`);
  if (on(f, "b.attr") && (f["b.attr.key"] ?? "").trim()) parts.push(`for customers whose ${f["b.attr.key"]!.trim()} is ${(f["b.attr.values"] ?? "…").trim() || "…"}`);
  if (on(f, "b.dates")) {
    const s = date(f, "b.starts_at", fmtDate);
    const e = date(f, "b.ends_at", fmtDate);
    if (s && e) parts.push(`from ${s} to ${e}`);
    else if (s) parts.push(`from ${s}`);
    else if (e) parts.push(`until ${e}`);
  }
  if (on(f, "b.max_total") && (f["b.max_total.max"] ?? "").trim()) parts.push(`up to ${f["b.max_total.max"]!.trim()} uses in total`);
  // bounds
  const cap = money(f, "cap");
  if (cap) parts.push(`at most ${cap} per order`);
  const budget = money(f, "budget");
  if (budget) parts.push(`up to ${budget} in total`);
  // codes
  const count = (f["codes.count"] ?? "").trim();
  if (count) {
    const uses = (f["codes.max_uses"] ?? "").trim();
    parts.push(`with ${count} ${uses === "1" ? "single-use codes" : uses ? `codes of ${uses} uses` : "codes"}`);
  }
  return parts.join(", ");
}
