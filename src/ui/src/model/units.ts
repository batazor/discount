// Human units at the edge (design.md, decision 2). The contract carries integer minor units
// and basis points; people enter and read decimals and percentages. Conversions are exact
// string arithmetic, never floats.

/** ISO 4217 minor-unit exponents that differ from 2. Unknown currencies use 2. */
export const EXPONENTS: Record<string, number> = {
  BIF: 0, CLP: 0, DJF: 0, GNF: 0, ISK: 0, JPY: 0, KMF: 0, KRW: 0, PYG: 0, RWF: 0, UGX: 0, VND: 0, VUV: 0, XAF: 0, XOF: 0, XPF: 0,
  BHD: 3, IQD: 3, JOD: 3, KWD: 3, LYD: 3, OMR: 3, TND: 3,
};

export function exponent(currency: string): number {
  return EXPONENTS[currency.trim().toUpperCase()] ?? 2;
}

const DECIMAL = /^(\d+)(?:[.,](\d+))?$/;

/** `"5.00"` + `EUR` → `500`; more decimals than the currency has → undefined. */
export function parseAmount(text: string, currency: string): number | undefined {
  const m = DECIMAL.exec(text.trim());
  if (!m) return undefined;
  const e = exponent(currency);
  const frac = m[2] ?? "";
  if (frac.length > e) return undefined;
  const n = Number(`${m[1]}${frac.padEnd(e, "0")}`);
  return Number.isSafeInteger(n) ? n : undefined;
}

/** `500` + `EUR` → `"5.00"`; the number only, for inputs. */
export function formatAmountInput(minor: number, currency: string): string {
  const e = exponent(currency);
  const sign = minor < 0 ? "-" : "";
  const digits = String(Math.abs(minor)).padStart(e + 1, "0");
  return e === 0 ? `${sign}${digits}` : `${sign}${digits.slice(0, -e)}.${digits.slice(-e)}`;
}

/** `500` + `EUR` → `"5.00 EUR"`, for display. */
export function formatAmount(minor: number, currency: string): string {
  return `${formatAmountInput(minor, currency)} ${currency}`;
}

/** `"12.5"` → `1250`; at most two decimals, 0–100 → undefined otherwise. */
export function parsePercent(text: string): number | undefined {
  const m = DECIMAL.exec(text.trim());
  if (!m) return undefined;
  const frac = m[2] ?? "";
  if (frac.length > 2) return undefined;
  const bps = Number(`${m[1]}${frac.padEnd(2, "0")}`);
  return bps <= 10000 ? bps : undefined;
}

/** `1250` → `"12.5"`, for inputs. */
export function formatPercentInput(bps: number): string {
  const whole = Math.floor(bps / 100);
  const frac = String(bps % 100).padStart(2, "0").replace(/0+$/, "");
  return frac ? `${whole}.${frac}` : String(whole);
}

/** `1250` → `"12.5 %"`, for display. */
export function formatBps(bps: number): string {
  return `${formatPercentInput(bps)} %`;
}
