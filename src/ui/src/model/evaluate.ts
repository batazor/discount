// The playground: an EvaluateRequest from form rows, and the PricingResult as rows to
// render. Nothing here prices anything; it only reshapes the service's answer.
import type { ClampReason, EvaluateRequest, Money, PricingResult, SkipReason } from "../api/types";
import { Form, moneyToFields, type Fields, type Parsed } from "./fields";
import { formatAmount } from "./units";

export function evaluateRequestFromForm(fields: Fields): Parsed<EvaluateRequest> {
  const f = new Form(fields);
  return f.finish({
    cart: {
      currency: f.str("cart.currency"),
      lines: f.rows("lines", "line_id").map((i) => ({
        line_id: f.str(`lines.${i}.line_id`),
        unit_price: f.money(`lines.${i}.unit_price`),
        quantity: f.int(`lines.${i}.quantity`, { min: 1 }),
      })),
      tags: f.list("cart.tags"),
      attributes: f.rows("attributes", "key").map((i) => ({
        key: f.str(`attributes.${i}.key`),
        value: f.str(`attributes.${i}.value`),
      })),
      coupon_codes: f.list("cart.coupon_codes"),
    },
    customer: {
      customer_id: f.optStr("customer.customer_id"),
      completed_orders: f.int("customer.completed_orders", { min: 0 }),
      uses: f.rows("uses", "promotion_id").map((i) => ({
        promotion_id: f.str(`uses.${i}.promotion_id`),
        count: f.int(`uses.${i}.count`, { min: 0 }),
      })),
      attributes: f.rows("customer_attributes", "key").map((i) => ({
        key: f.str(`customer_attributes.${i}.key`),
        value: f.str(`customer_attributes.${i}.value`),
      })),
      allowlisted: f.list("customer.allowlisted"),
    },
  });
}

export function evaluateRequestToFields(r: EvaluateRequest): Fields {
  const out: Fields = {
    "cart.currency": r.cart.currency,
    "cart.tags": r.cart.tags.join(", "),
    "cart.coupon_codes": r.cart.coupon_codes.join(", "),
    "customer.customer_id": r.customer.customer_id ?? "",
    "customer.completed_orders": String(r.customer.completed_orders),
    "customer.allowlisted": r.customer.allowlisted.join(", "),
  };
  r.cart.lines.forEach((l, i) => {
    out[`lines.${i}.line_id`] = l.line_id;
    Object.assign(out, moneyToFields(l.unit_price, `lines.${i}.unit_price`));
    out[`lines.${i}.quantity`] = String(l.quantity);
  });
  r.cart.attributes.forEach((a, i) => {
    out[`attributes.${i}.key`] = a.key;
    out[`attributes.${i}.value`] = a.value;
  });
  r.customer.uses.forEach((u, i) => {
    out[`uses.${i}.promotion_id`] = u.promotion_id;
    out[`uses.${i}.count`] = String(u.count);
  });
  r.customer.attributes.forEach((a, i) => {
    out[`customer_attributes.${i}.key`] = a.key;
    out[`customer_attributes.${i}.value`] = a.value;
  });
  return out;
}

/** The sample cart of deploy/smoke.sh, the playground's starting point. */
export const SAMPLE_REQUEST: EvaluateRequest = {
  cart: {
    currency: "EUR",
    lines: [
      { line_id: "L1", unit_price: { amount: 1999, currency: "EUR" }, quantity: 2 },
      { line_id: "L2", unit_price: { amount: 501, currency: "EUR" }, quantity: 1 },
    ],
    tags: ["#ios"],
    attributes: [],
    coupon_codes: [],
  },
  customer: { customer_id: "smoke", completed_orders: 0, uses: [], attributes: [], allowlisted: [] },
};

/** Human form, `5.00 EUR`; null → `—`. */
export function fmtMoney(m: Money | null | undefined): string {
  return m ? formatAmount(m.amount, m.currency) : "—";
}

export function skipReasonText(r: SkipReason): string {
  switch (r.type) {
    case "NotEligible":
      return `NotEligible (rule #${r.rule_index})`;
    case "ExclusiveApplied":
      return `ExclusiveApplied (winner ${r.winner})`;
    case "FirstMatchTaken":
      return `FirstMatchTaken (winner ${r.winner})`;
  }
}

export type ResultRows = {
  totals: { label: string; value: string }[];
  lines: { line_id: string; subtotal: string; discount: string; total: string }[];
  applied: { promotion_id: string; discount: string; clamped_by: string; clamp: ClampReason["type"] | null; allocation: string }[];
  skipped: { promotion_id: string; reason: string; skip: SkipReason["type"] }[];
};

export function resultRows(r: PricingResult): ResultRows {
  return {
    totals: [
      { label: "Currency", value: r.currency },
      { label: "Original subtotal", value: fmtMoney(r.original_subtotal) },
      { label: "Total discount", value: fmtMoney(r.total_discount) },
      { label: "Final total", value: fmtMoney(r.final_total) },
    ],
    lines: r.lines.map((l) => ({
      line_id: l.line_id,
      subtotal: fmtMoney(l.subtotal),
      discount: fmtMoney(l.discount),
      total: fmtMoney(l.total),
    })),
    applied: r.applied.map((a) => ({
      promotion_id: a.promotion_id,
      discount: fmtMoney(a.discount),
      clamped_by: a.clamped_by?.type ?? "—",
      clamp: a.clamped_by?.type ?? null,
      allocation: a.allocation.map((x) => `${x.line_id} ${formatAmount(x.amount.amount, x.amount.currency)}`).join(", "),
    })),
    skipped: r.skipped.map((s) => ({ promotion_id: s.promotion_id, reason: skipReasonText(s.reason), skip: s.reason.type })),
  };
}

/** Pasted JSON: parsed as-is so the API's own `400` for unknown keys is what the screen shows. */
export function parseJson(text: string): Parsed<unknown> {
  try {
    return { ok: true, value: JSON.parse(text) };
  } catch (e) {
    return { ok: false, errors: [{ field: "json", message: (e as Error).message }] };
  }
}
