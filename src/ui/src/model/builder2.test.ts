import { describe, expect, it } from "vitest";
import type { Promotion, Rule } from "../api/types";
import { applyBuilder, builderErrors, builderFromRules, leftoverFields } from "./builder";
import { campaignDraftFromForm } from "./campaign";
import { promotionFromForm } from "./promotion";
import { describeConditions } from "./summary";
import { fromCampaign } from "./templates";

const base = { builder: "1", promotion_id: "X", group_id: "g", priority: "0", "action.type": "OrderPercentage", "action.percent": "10" };

describe("spec admin-ui: rule builder on the rules tab", () => {
  it("Round trip: expressible rules become conditions, the rest stay advanced, saving keeps all three", () => {
    const rules: Rule[] = [{ type: "MaxCompletedOrders", max: 0 }, { type: "RequiresCoupon" }, { type: "TagsMatch", all: [], any: [], none: ["#partner"] }];
    const { fields, leftover } = builderFromRules(rules);
    expect(fields).toEqual({ "b.first_order": "on", "b.needs_code": "on" });
    expect(leftover).toEqual([rules[2]]);
    const form = { ...base, ...fields, ...leftoverFields(leftover) };
    const saved = promotionFromForm(applyBuilder(form));
    expect(saved.ok && saved.value.rules).toEqual([{ type: "MaxCompletedOrders", max: 0 }, { type: "RequiresCoupon" }, rules[2]]);
  });
  it("Both bounds become one OrderTotalRange", () => {
    const r = campaignDraftFromForm({ ...base, "b.min_total": "on", "b.min_total.amount": "50.00", "b.min_total.currency": "EUR", "b.max_order": "on", "b.max_order.amount": "500.00", "b.max_order.currency": "EUR" });
    expect(r.ok && r.value.promotion.rules).toEqual([{ type: "OrderTotalRange", min: { amount: 5000, currency: "EUR" }, max: { amount: 50000, currency: "EUR" } }]);
  });
  it("End before start is a field error and nothing is built", () => {
    const r = campaignDraftFromForm({ ...base, "b.dates": "on", "b.starts_at": "2026-11-30T00:00", "b.ends_at": "2026-11-01T00:00" });
    expect(r).toMatchObject({ ok: false, errors: [{ field: "b.ends_at", message: "ends before it starts" }] });
    expect(builderErrors({ "b.min_total": "on", "b.min_total.amount": "600", "b.min_total.currency": "EUR", "b.max_order": "on", "b.max_order.amount": "500", "b.max_order.currency": "EUR" })).toEqual([{ field: "b.max_order.amount", message: "below the minimum" }]);
    expect(builderErrors({ "b.attr_range": "on", "b.attr_range.key": "k" })).toEqual([{ field: "b.attr_range.min", message: "give at least one bound" }]);
  });
  it("new conditions map to their rules and back", () => {
    const rules: Rule[] = [
      { type: "AttributeIn", key: "hotel_kind", values: ["resort"] },
      { type: "AttributeDateWindow", key: "checkin", starts_at: 1760227200, ends_at: null },
      { type: "CustomerAttributeRange", key: "app_orders", min: null, max: 0 },
      { type: "OrderTotalRange", min: null, max: { amount: 9900, currency: "EUR" } },
    ];
    const { fields, leftover } = builderFromRules(rules);
    expect(leftover).toEqual([]);
    const back = promotionFromForm(applyBuilder({ ...base, ...fields }));
    expect(back.ok && back.value.rules.map((r) => r.type).sort()).toEqual(["AttributeDateWindow", "AttributeIn", "CustomerAttributeRange", "OrderTotalRange"]);
    expect(back.ok && back.value.rules.find((r) => r.type === "OrderTotalRange")).toEqual(rules[3]);
  });
});

describe("spec admin-ui: who gets it", () => {
  it("Sentence of conditions", () => {
    expect(describeConditions({ "b.first_order": "on", "b.needs_code": "on" })).toBe("for first orders only, with a code");
    expect(describeConditions({ "b.max_order": "on", "b.max_order.amount": "99.00", "b.max_order.currency": "EUR", "b.attr_range": "on", "b.attr_range.key": "app_orders", "b.attr_range.max": "0" })).toBe("for orders up to 99.00 EUR, for customers whose app_orders is at most 0");
  });
});

describe("spec admin-ui: duplication", () => {
  it("Duplicate: fields from an existing campaign, id empty, title marked as a copy", () => {
    const p: Promotion = { promotion_id: "SUMMER", group_id: "g", priority: 3, exclusive: false, cap: null, remaining_budget: { amount: 1000, currency: "EUR" }, uses_total: 9, rounding: null, rules: [{ type: "MaxCompletedOrders", max: 0 }, { type: "RequiresCoupon" }], action: { type: "OrderPercentage", bps: 1000 } };
    const f = fromCampaign(p, { title: "Summer", description: "d", source: "hotels", labels: { channel: "app" } });
    expect(f).toMatchObject({ "action.percent": "10", "b.first_order": "on", "b.needs_code": "on", title: "Summer copy", source: "hotels", labels: "channel=app", "budget.currency": "EUR", group_id: "g", priority: "3", copied_from: "SUMMER" });
    expect(f.promotion_id).toBeUndefined();
    expect(fromCampaign(p, null).title).toBe("SUMMER copy");
  });
});
