// Form fields are flat `name → string` maps (what a plain <form> posts). Nested shapes use
// dotted names: `cap.amount`, `rules.0.type`. `Form` reads them with validation and collects
// every error so a screen can show all of them at once.
import type { Money } from "../api/types";
import { parseTimestamp } from "./time";
import { exponent, formatAmountInput, parseAmount, parsePercent } from "./units";

export type Fields = Record<string, string>;
export type FieldError = { field: string; message: string };
export type Parsed<T> = { ok: true; value: T } | { ok: false; errors: FieldError[] };

export function fieldsOf(fd: FormData): Fields {
  const out: Fields = {};
  for (const [k, v] of fd.entries()) if (typeof v === "string") out[k] = v;
  return out;
}

const INT = /^-?\d+$/;

export class Form {
  readonly errors: FieldError[] = [];
  constructor(readonly fields: Fields) {}

  fail(field: string, message: string): void {
    this.errors.push({ field, message });
  }

  has(key: string): boolean {
    return key in this.fields;
  }

  raw(key: string): string {
    return (this.fields[key] ?? "").trim();
  }

  str(key: string): string {
    const v = this.raw(key);
    if (v === "") this.fail(key, "required");
    return v;
  }

  optStr(key: string): string | null {
    const v = this.raw(key);
    return v === "" ? null : v;
  }

  /** An integer. Money and counters are integers in the contract; decimals are refused here. */
  int(key: string, opts: { min?: number; max?: number } = {}): number {
    const v = this.raw(key);
    if (!INT.test(v)) {
      this.fail(key, v === "" ? "required" : "must be an integer (minor units, no decimals)");
      return 0;
    }
    const n = Number(v);
    if (!Number.isSafeInteger(n)) {
      this.fail(key, "out of range");
      return 0;
    }
    if (opts.min !== undefined && n < opts.min) this.fail(key, `must be ≥ ${opts.min}`);
    if (opts.max !== undefined && n > opts.max) this.fail(key, `must be ≤ ${opts.max}`);
    return n;
  }

  optInt(key: string, opts: { min?: number; max?: number } = {}): number | null {
    return this.raw(key) === "" ? null : this.int(key, opts);
  }

  bool(key: string): boolean {
    const v = this.raw(key).toLowerCase();
    return v === "on" || v === "true" || v === "1";
  }

  enum<T extends string>(key: string, values: readonly T[]): T {
    const v = this.raw(key);
    if ((values as readonly string[]).includes(v)) return v as T;
    this.fail(key, v === "" ? "required" : `must be one of ${values.join(", ")}`);
    return values[0]!;
  }

  optEnum<T extends string>(key: string, values: readonly T[]): T | null {
    return this.raw(key) === "" ? null : this.enum(key, values);
  }

  /** `prefix.amount` as a decimal in the currency's units (`5.00`) and `prefix.currency`. */
  money(prefix: string): Money {
    const currency = this.str(`${prefix}.currency`);
    const raw = this.raw(`${prefix}.amount`);
    if (raw === "") {
      this.fail(`${prefix}.amount`, "required");
      return { amount: 0, currency };
    }
    const amount = currency === "" ? undefined : parseAmount(raw, currency);
    if (amount === undefined) {
      if (currency !== "") {
        const e = exponent(currency);
        this.fail(`${prefix}.amount`, e === 0 ? `${currency} has no decimals` : `must be an amount with at most ${e} decimals, e.g. ${formatAmountInput(5 * 10 ** e, currency)}`);
      }
      return { amount: 0, currency };
    }
    return { amount, currency };
  }

  /** A percentage (`12.5`) → basis points (`1250`). */
  percent(key: string): number {
    const raw = this.raw(key);
    const bps = parsePercent(raw);
    if (bps === undefined) {
      this.fail(key, raw === "" ? "required" : "must be a percentage between 0 and 100 with at most two decimals");
      return 0;
    }
    return bps;
  }

  optPercent(key: string): number | null {
    return this.raw(key) === "" ? null : this.percent(key);
  }

  /** null when both parts are empty. */
  optMoney(prefix: string): Money | null {
    if (this.raw(`${prefix}.amount`) === "" && this.raw(`${prefix}.currency`) === "") return null;
    return this.money(prefix);
  }

  /**
   * A datetime-local value is read in the viewer's zone when the form carries
   * `tz_offset_minutes` (filled by script, as `Date.getTimezoneOffset()` reports it), else as
   * UTC. A raw integer is Unix seconds either way.
   */
  timestamp(key: string): number | null {
    const raw = this.raw(key);
    const t = parseTimestamp(raw);
    if (t === undefined) {
      this.fail(key, "must be Unix seconds or YYYY-MM-DDTHH:mm");
      return null;
    }
    if (t === null || /^-?\d+$/.test(raw)) return t;
    const off = this.raw("tz_offset_minutes");
    return /^-?\d+$/.test(off) ? t + Number(off) * 60 : t;
  }

  /** Comma- or newline-separated strings, trimmed, empties dropped. */
  list(key: string): string[] {
    return this.raw(key)
      .split(/[,\n]/)
      .map((s) => s.trim())
      .filter((s) => s !== "");
  }

  /**
   * Indices `i` for which some `prefix.i.*` field exists, ascending (rows may have gaps).
   * With `key`, rows whose `prefix.i.key` is empty are skipped: a blank row is no row.
   */
  rows(prefix: string, key?: string): number[] {
    const re = new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\.(\\d+)\\.`);
    const idx = new Set<number>();
    for (const k of Object.keys(this.fields)) {
      const m = re.exec(k);
      if (m) idx.add(Number(m[1]));
    }
    return [...idx].sort((a, b) => a - b).filter((i) => key === undefined || this.raw(`${prefix}.${i}.${key}`) !== "");
  }

  finish<T>(value: T): Parsed<T> {
    return this.errors.length ? { ok: false, errors: this.errors } : { ok: true, value };
  }
}

/** Flatten a nested JSON value into dotted fields (arrays by index, null → ""). */
export function flatten(value: unknown, prefix = "", out: Fields = {}): Fields {
  if (value === null || value === undefined) {
    if (prefix) out[prefix] = "";
  } else if (Array.isArray(value)) {
    value.forEach((v, i) => flatten(v, prefix ? `${prefix}.${i}` : String(i), out));
  } else if (typeof value === "object") {
    for (const [k, v] of Object.entries(value)) flatten(v, prefix ? `${prefix}.${k}` : k, out);
  } else {
    out[prefix] = String(value);
  }
  return out;
}

/** Money → decimal input fields (`prefix.amount` = `"5.00"`, `prefix.currency`); null → empty. */
export function moneyToFields(m: Money | null | undefined, prefix: string): Fields {
  if (!m) return { [`${prefix}.amount`]: "", [`${prefix}.currency`]: "" };
  return { [`${prefix}.amount`]: formatAmountInput(m.amount, m.currency), [`${prefix}.currency`]: m.currency };
}
