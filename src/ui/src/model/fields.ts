// Form fields are flat `name → string` maps (what a plain <form> posts). Nested shapes use
// dotted names: `cap.amount`, `rules.0.type`. `Form` reads them with validation and collects
// every error so a screen can show all of them at once.
import type { Money } from "../api/types";
import { parseTimestamp } from "./time";

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

  /** `prefix.amount` (integer minor units) and `prefix.currency`. */
  money(prefix: string): Money {
    return { amount: this.int(`${prefix}.amount`, { min: 0 }), currency: this.str(`${prefix}.currency`) };
  }

  /** null when both parts are empty. */
  optMoney(prefix: string): Money | null {
    if (this.raw(`${prefix}.amount`) === "" && this.raw(`${prefix}.currency`) === "") return null;
    return this.money(prefix);
  }

  timestamp(key: string): number | null {
    const t = parseTimestamp(this.raw(key));
    if (t === undefined) {
      this.fail(key, "must be Unix seconds or YYYY-MM-DDTHH:mm (UTC)");
      return null;
    }
    return t;
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
