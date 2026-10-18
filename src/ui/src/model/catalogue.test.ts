import { describe, expect, it } from "vitest";
import type { Coupon, ExchangeRate, GrantRule, PricingPolicy, StackingGroup } from "../api/types";
import {
  couponFromForm,
  couponToFields,
  exchangeRateFromForm,
  exchangeRateToFields,
  generateRequestFromForm,
  grantRequestFromForm,
  grantRuleFromForm,
  grantRuleToFields,
  groupFromForm,
  groupToFields,
  policyFromForm,
  policyToFields,
} from "./catalogue";

describe("spec admin-ui: derived fields are read-only", () => {
  it("Uses are shown, not sent: a coupon saved with uses = 3 goes out with uses = 0", () => {
    const fields = { ...couponToFields({ code: "ONCE", promotion_id: "P3", starts_at: null, ends_at: null, customer_id: null, max_uses: 1, uses: 3 }), uses: "3" };
    const r = couponFromForm(fields);
    expect(r.ok && r.value.uses).toBe(0);
    expect(r.ok && r.value.max_uses).toBe(1);
  });
});

describe("round trips", () => {
  it("policy, with and without an order cap", () => {
    const capped: PricingPolicy = {
      rounding: { type: "HalfUp" },
      stacking: { type: "Additive" },
      order_cap: { amount: { amount: 5000, currency: "EUR" }, bps: 2500 },
    };
    const open: PricingPolicy = { rounding: { type: "Down" }, stacking: { type: "Sequential" }, order_cap: { amount: null, bps: null } };
    for (const p of [capped, open]) expect(policyFromForm(policyToFields(p))).toEqual({ ok: true, value: p });
  });

  it("group", () => {
    const g: StackingGroup = { group_id: "g", priority: -1, mode: { type: "FirstMatch" } };
    expect(groupFromForm(groupToFields(g))).toEqual({ ok: true, value: g });
  });

  it("coupon with a window and a binding", () => {
    const c: Coupon = { code: "WELCOME", promotion_id: "P1", starts_at: 1760227200, ends_at: 1760313600, customer_id: "c1", max_uses: null, uses: 0 };
    expect(couponFromForm(couponToFields(c))).toEqual({ ok: true, value: c });
  });

  it("grant rule", () => {
    const r: GrantRule = { rule_id: "cancel-10", trigger: "cancel", promotion_id: "P3", priority: 10, validity_seconds: 86400, max_uses: 1 };
    expect(grantRuleFromForm(grantRuleToFields(r))).toEqual({ ok: true, value: r });
    expect(grantRuleFromForm({ ...grantRuleToFields(r), validity_seconds: "0" }).ok).toBe(false);
  });

  it("exchange rate", () => {
    const x: ExchangeRate = { from: "USD", to: "EUR", numerator: 92, denominator: 100 };
    expect(exchangeRateFromForm(exchangeRateToFields(x))).toEqual({ ok: true, value: x });
  });

  it("generation and grant requests", () => {
    expect(generateRequestFromForm({ promotion_id: "P3", count: "5", prefix: "SPRING-", max_uses: "1" })).toEqual({
      ok: true,
      value: { promotion_id: "P3", count: 5, prefix: "SPRING-", starts_at: null, ends_at: null, max_uses: 1, customer_id: null },
    });
    expect(generateRequestFromForm({ promotion_id: "P3", count: "0", prefix: "" }).ok).toBe(false);
    expect(grantRequestFromForm({ trigger: "review", customer_id: "c1", order_id: "o1" })).toEqual({
      ok: true,
      value: { trigger: "review", customer_id: "c1", order_id: "o1" },
    });
  });
});
