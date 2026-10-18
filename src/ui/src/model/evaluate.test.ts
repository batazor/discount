import { describe, expect, it } from "vitest";
import type { EvaluateRequest, PricingResult } from "../api/types";
import { evaluateRequestFromForm, evaluateRequestToFields, parseJson, resultRows, SAMPLE_REQUEST, skipReasonText } from "./evaluate";

describe("evaluate request", () => {
  it("round trips the sample cart and a fuller one", () => {
    const full: EvaluateRequest = {
      cart: {
        currency: "EUR",
        lines: [{ line_id: "L1", unit_price: { amount: 1999, currency: "EUR" }, quantity: 2 }],
        tags: ["#ios", "vip"],
        attributes: [{ key: "tier", value: "gold" }],
        coupon_codes: ["WELCOME"],
      },
      customer: {
        customer_id: null,
        completed_orders: 3,
        uses: [{ promotion_id: "P1", count: 1 }],
        attributes: [{ key: "tier", value: "gold" }],
        allowlisted: ["P1", "P2"],
      },
    };
    for (const r of [SAMPLE_REQUEST, full]) expect(evaluateRequestFromForm(evaluateRequestToFields(r))).toEqual({ ok: true, value: r });
  });

  it("keeps the currency as typed so the API's InvalidCurrency is what the screen shows", () => {
    const r = evaluateRequestFromForm({ ...evaluateRequestToFields(SAMPLE_REQUEST), "cart.currency": "eur" });
    expect(r.ok && r.value.cart.currency).toBe("eur");
  });

  it("pasted JSON is passed through, invalid JSON is an error", () => {
    expect(parseJson('{"a":1}')).toEqual({ ok: true, value: { a: 1 } });
    expect(parseJson("{").ok).toBe(false);
  });
});

describe("result rows", () => {
  const result: PricingResult = {
    currency: "EUR",
    original_subtotal: { amount: 4499, currency: "EUR" },
    total_discount: { amount: 749, currency: "EUR" },
    final_total: { amount: 3750, currency: "EUR" },
    lines: [{ line_id: "L1", subtotal: { amount: 3998, currency: "EUR" }, discount: { amount: 666, currency: "EUR" }, total: { amount: 3332, currency: "EUR" } }],
    applied: [
      { promotion_id: "P1", discount: { amount: 449, currency: "EUR" }, clamped_by: null, allocation: [{ line_id: "L1", amount: { amount: 399, currency: "EUR" } }] },
      { promotion_id: "P2", discount: { amount: 300, currency: "EUR" }, clamped_by: { type: "Cap" }, allocation: [] },
    ],
    skipped: [
      { promotion_id: "P3", reason: { type: "NotEligible", rule_index: 0 } },
      { promotion_id: "P4", reason: { type: "ExclusiveApplied", winner: "P1" } },
    ],
  };

  it("renders totals, applied with clamps and allocation, skipped with reasons", () => {
    const rows = resultRows(result);
    expect(rows.totals.map((t) => t.value)).toEqual(["EUR", "4499 EUR", "749 EUR", "3750 EUR"]);
    expect(rows.applied).toEqual([
      { promotion_id: "P1", discount: "449 EUR", clamped_by: "—", clamp: null, allocation: "L1 399" },
      { promotion_id: "P2", discount: "300 EUR", clamped_by: "Cap", clamp: "Cap", allocation: "" },
    ]);
    expect(rows.skipped).toEqual([
      { promotion_id: "P3", reason: "NotEligible (rule #0)", skip: "NotEligible" },
      { promotion_id: "P4", reason: "ExclusiveApplied (winner P1)", skip: "ExclusiveApplied" },
    ]);
    expect(skipReasonText({ type: "FirstMatchTaken", winner: "P9" })).toBe("FirstMatchTaken (winner P9)");
  });
});
