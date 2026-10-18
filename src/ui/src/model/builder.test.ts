import { describe, expect, it } from "vitest";
import { applyBuilder, rulesFromBuilder } from "./builder";
import { campaignDraftFromForm } from "./campaign";
import { summarize } from "./summary";
import { TEMPLATES } from "./templates";

const base = { builder: "1", promotion_id: "W", group_id: "g", priority: "0", "action.type": "OrderPercentage", "action.percent": "10" };

describe("spec admin-ui: rule builder in plain words", () => {
  it("First order and dates", () => {
    const r = campaignDraftFromForm({ ...base, "b.first_order": "on", "b.dates": "on", "b.starts_at": "2026-11-01T00:00", "b.ends_at": "" });
    expect(r.ok && r.value.promotion.rules).toEqual([
      { type: "DateWindow", starts_at: Date.UTC(2026, 10, 1) / 1000, ends_at: null },
      { type: "MaxCompletedOrders", max: 0 },
    ]);
  });
  it("Needs a code generates codes", () => {
    const r = campaignDraftFromForm({ ...base, "b.needs_code": "on", "codes.count": "5", "codes.prefix": "W-", "codes.max_uses": "1" });
    expect(r.ok && r.value.promotion.rules).toEqual([{ type: "RequiresCoupon" }]);
    expect(r.ok && r.value.generate?.count).toBe(5);
  });
  it("every condition maps to its rule, in a fixed order", () => {
    const f = rulesFromBuilder({ "b.allowlist": "on", "b.needs_code": "on", "b.max_total": "on", "b.max_total.max": "100", "b.attr": "on", "b.attr.key": "tier", "b.attr.values": "gold, silver", "b.tags": "on", "b.tags_any": "#ios", "b.once": "on", "b.first_order": "on", "b.min_total": "on", "b.min_total.amount": "50.00", "b.min_total.currency": "EUR", "b.dates": "on", "b.starts_at": "", "b.ends_at": "1760313600" });
    expect([0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => f[`rules.${i}.type`])).toEqual(["DateWindow", "OrderTotalRange", "MaxCompletedOrders", "MaxUsesPerCustomer", "TagsMatch", "CustomerAttributeIn", "MaxUsesTotal", "RequiresCoupon", "RequiresAllowlist"]);
    expect(f["rules.1.min.amount"]).toBe("50.00");
    expect(f["rules.5.values"]).toBe("gold, silver");
  });
  it("without the builder flag the raw rules stay", () => {
    expect(applyBuilder({ "rules.0.type": "RequiresCoupon", "b.first_order": "on" })).toEqual({ "rules.0.type": "RequiresCoupon", "b.first_order": "on" });
    // With the builder, raw rows are advanced rules and follow the builder's rules.
    expect(applyBuilder({ builder: "1", "rules.0.type": "RequiresCoupon", "b.first_order": "on" })).toEqual({ builder: "1", "b.first_order": "on", "rules.0.type": "MaxCompletedOrders", "rules.0.max": "0", "rules.1.type": "RequiresCoupon" });
  });
});

describe("spec admin-ui: live summary", () => {
  it("Sentence", () => {
    const s = summarize({ "action.type": "OrderPercentage", "action.percent": "10", "b.first_order": "on", "b.dates": "on", "b.starts_at": "2026-11-01T00:00", "b.ends_at": "2026-11-30T23:59", "budget.amount": "1000.00", "budget.currency": "EUR", "codes.count": "500", "codes.max_uses": "1" });
    expect(s).toBe("10 % off the order, for first orders only, from 1 Nov 2026 to 30 Nov 2026, up to 1 000.00 EUR in total, with 500 single-use codes");
  });
  it("fixed amount, cap, open dates, other codes", () => {
    expect(summarize({ "action.type": "OrderFixedAmount", "action.amount.amount": "5.00", "action.amount.currency": "EUR", "cap.amount": "20", "cap.currency": "EUR", "b.dates": "on", "b.ends_at": "2026-12-31T00:00", "codes.count": "10", "codes.max_uses": "3" }))
      .toBe("5.00 EUR off the order, until 31 Dec 2026, at most 20.00 EUR per order, with 10 codes of 3 uses");
    expect(summarize({})).toBe("… % off the order");
  });
});

describe("templates", () => {
  it("each produces a valid draft once an id and a group are given", () => {
    for (const t of TEMPLATES) {
      const r = campaignDraftFromForm({ builder: "1", promotion_id: "T", group_id: "g", priority: "0", ...t.fields, ...(t.fields["b.dates"] ? { "b.starts_at": "", "b.ends_at": "" } : {}) });
      expect(r.ok, t.id).toBe(true);
    }
  });
});
