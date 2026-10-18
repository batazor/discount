import { describe, expect, it } from "vitest";
import type { Promotion, Rule } from "../api/types";
import { promotionFromForm, promotionToFields, ruleFromForm, ruleToFields, RULE_TYPES } from "./promotion";
import { Form } from "./fields";

const base = {
  promotion_id: "P9",
  group_id: "g",
  priority: "10",
  "action.type": "OrderPercentage",
  "action.percent": "10",
};

describe("spec admin-ui: rules and actions are edited structurally", () => {
  it("Add a date window rule: starts_at set, ends_at empty → null", () => {
    const r = promotionFromForm({
      ...base,
      "rules.0.type": "DateWindow",
      "rules.0.starts_at": "2026-10-18T00:00",
      "rules.0.ends_at": "",
    });
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.value.rules).toEqual([
      { type: "DateWindow", starts_at: Date.UTC(2026, 9, 18) / 1000, ends_at: null },
    ]);
  });

  it("a raw integer timestamp passes through unchanged", () => {
    const f = new Form({ "r.type": "DateWindow", "r.starts_at": "1760227200", "r.ends_at": "" });
    expect(ruleFromForm(f, "r")).toEqual({ type: "DateWindow", starts_at: 1760227200, ends_at: null });
    expect(f.errors).toEqual([]);
  });

  it("Fixed amount in minor units: 5.00 EUR is sent as 500", () => {
    const r = promotionFromForm({
      ...base,
      "action.type": "OrderFixedAmount",
      "action.amount.amount": "5.00",
      "action.amount.currency": "EUR",
    });
    expect(r.ok && r.value.action).toEqual({ type: "OrderFixedAmount", amount: { amount: 500, currency: "EUR" } });
  });

  it("Currency without decimals: 500.5 JPY is refused as a field error", () => {
    expect(promotionFromForm({ ...base, "action.type": "OrderFixedAmount", "action.amount.amount": "500", "action.amount.currency": "JPY" })).toMatchObject({ ok: true, value: { action: { amount: { amount: 500, currency: "JPY" } } } });
    const r = promotionFromForm({
      ...base,
      "action.type": "OrderFixedAmount",
      "action.amount.amount": "500.5",
      "action.amount.currency": "JPY",
    });
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.errors[0]?.field).toBe("action.amount.amount");
  });

  it("Create a promotion: the body is the contract shape with derived fields zeroed", () => {
    const r = promotionFromForm(base);
    expect(r).toEqual({
      ok: true,
      value: {
        promotion_id: "P9",
        group_id: "g",
        priority: 10,
        exclusive: false,
        cap: null,
        remaining_budget: null,
        uses_total: 0,
        rounding: null,
        rules: [],
        action: { type: "OrderPercentage", bps: 1000 },
      },
    });
  });

  it("Percent to basis points: 12.5 → 1250, 12.345 refused", () => {
    expect(promotionFromForm({ ...base, "action.percent": "12.5" })).toMatchObject({ ok: true, value: { action: { type: "OrderPercentage", bps: 1250 } } });
    const r = promotionFromForm({ ...base, "action.percent": "12.345" });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors[0]?.field).toBe("action.percent");
  });

  it("collects every error at once", () => {
    const r = promotionFromForm({ "action.type": "OrderPercentage", "action.percent": "200" });
    expect(r.ok).toBe(false);
    if (r.ok) return;
    expect(r.errors.map((e) => e.field).sort()).toEqual(["action.percent", "group_id", "priority", "promotion_id"]);
  });

  it("a datetime-local is read in the viewer's zone when the offset is sent", () => {
    const r = promotionFromForm({ ...base, "rules.0.type": "DateWindow", "rules.0.starts_at": "2026-10-18T00:00", "rules.0.ends_at": "", tz_offset_minutes: "-120" });
    expect(r.ok && r.value.rules[0]).toEqual({ type: "DateWindow", starts_at: Date.UTC(2026, 9, 18) / 1000 - 7200, ends_at: null });
  });
});

const everyRule: Rule[] = [
  { type: "DateWindow", starts_at: 1760227200, ends_at: null },
  { type: "OrderTotalRange", min: { amount: 1000, currency: "EUR" }, max: null },
  { type: "TagsMatch", all: ["#ios"], any: ["a", "b"], none: [] },
  { type: "AttributeIn", key: "tier", values: ["gold", "silver"] },
  { type: "AttributeDateWindow", key: "checkin", starts_at: null, ends_at: 1760313600 },
  { type: "MaxCompletedOrders", max: 0 },
  { type: "MaxUsesPerCustomer", max: 2 },
  { type: "MaxUsesTotal", max: 100 },
  { type: "RequiresCoupon" },
  { type: "CustomerAttributeIn", key: "tier", values: ["gold"] },
  { type: "CustomerAttributeRange", key: "completed_app_orders", min: null, max: 0 },
  { type: "RequiresAllowlist" },
];

describe("round trips", () => {
  it("covers every Rule variant of the contract", () => {
    expect(everyRule.map((r) => r.type)).toEqual([...RULE_TYPES]);
    for (const rule of everyRule) {
      const f = new Form(ruleToFields(rule, "r"));
      expect(ruleFromForm(f, "r")).toEqual(rule);
      expect(f.errors).toEqual([]);
    }
  });

  it("promotion → fields → promotion, with derived fields reset", () => {
    const p: Promotion = {
      promotion_id: "P1",
      group_id: "g",
      priority: 5,
      exclusive: true,
      cap: { amount: 300, currency: "EUR" },
      remaining_budget: { amount: 999, currency: "EUR" },
      uses_total: 7,
      rounding: { type: "HalfEven" },
      rules: everyRule,
      action: { type: "OrderFixedAmount", amount: { amount: 500, currency: "EUR" } },
    };
    const back = promotionFromForm(promotionToFields(p));
    expect(back).toEqual({ ok: true, value: { ...p, remaining_budget: null, uses_total: 0 } });
  });

  it("a blank rule row (type not chosen) is no rule", () => {
    const r = promotionFromForm({ ...base, "rules.0.type": "", "rules.0.max": "" });
    expect(r.ok && r.value.rules).toEqual([]);
  });

  it("rows may have gaps (a row removed in the browser)", () => {
    const r = promotionFromForm({ ...base, "rules.0.type": "RequiresCoupon", "rules.2.type": "MaxUsesTotal", "rules.2.max": "3" });
    expect(r.ok && r.value.rules).toEqual([{ type: "RequiresCoupon" }, { type: "MaxUsesTotal", max: 3 }]);
  });
});
