import { describe, expect, it } from "vitest";
import type { Promotion } from "../api/types";
import { assembleCampaigns, campaignStatus, describeAction } from "./campaign";

const NOW = 1_760_227_200;
const DAY = 86400;
const base: Promotion = {
  promotion_id: "P1", group_id: "g", priority: 0, exclusive: false, cap: null, remaining_budget: null, uses_total: 0, rounding: null, rules: [],
  action: { type: "OrderPercentage", bps: 1000 },
};
const withRules = (rules: Promotion["rules"], extra: Partial<Promotion> = {}): Promotion => ({ ...base, ...extra, rules });

describe("spec admin-ui: campaign status", () => {
  it("Exhausted by budget", () => {
    expect(campaignStatus({ ...base, remaining_budget: { amount: 0, currency: "EUR" } }, NOW)).toBe("exhausted");
  });
  it("Ended wins over exhausted", () => {
    const p = withRules([{ type: "DateWindow", starts_at: null, ends_at: NOW - DAY }], { remaining_budget: { amount: 0, currency: "EUR" } });
    expect(campaignStatus(p, NOW)).toBe("ended");
  });
  it("No window, budget left → live", () => {
    expect(campaignStatus({ ...base, remaining_budget: { amount: 500, currency: "EUR" } }, NOW)).toBe("live");
    expect(campaignStatus(base, NOW)).toBe("live");
  });
  it("scheduled when a window starts later; exhausted by total uses", () => {
    expect(campaignStatus(withRules([{ type: "DateWindow", starts_at: NOW + DAY, ends_at: null }]), NOW)).toBe("scheduled");
    expect(campaignStatus(withRules([{ type: "MaxUsesTotal", max: 3 }], { uses_total: 3 }), NOW)).toBe("exhausted");
    expect(campaignStatus(withRules([{ type: "MaxUsesTotal", max: 3 }], { uses_total: 2 }), NOW)).toBe("live");
  });
});

describe("assembleCampaigns", () => {
  it("Ending soon: a window ending in three days", () => {
    const p = withRules([{ type: "DateWindow", starts_at: null, ends_at: NOW + 3 * DAY }]);
    const far = withRules([{ type: "DateWindow", starts_at: null, ends_at: NOW + 30 * DAY }], { promotion_id: "P2" });
    const [a, b] = assembleCampaigns([p, far], [{ promotion_id: "P1", metadata: { title: "Summer", description: "", source: "hotels", labels: {} } }], [
      { code: "A", promotion_id: "P1", starts_at: null, ends_at: null, customer_id: null, max_uses: 1, uses: 0 },
      { code: "B", promotion_id: "P1", starts_at: null, ends_at: null, customer_id: null, max_uses: 1, uses: 0 },
    ], NOW);
    expect(a).toMatchObject({ status: "live", endingSoon: true, endsAt: NOW + 3 * DAY, codes: [{ code: "A" }, { code: "B" }], metadata: { title: "Summer" } });
    expect(b).toMatchObject({ status: "live", endingSoon: false, codes: [], metadata: null });
  });
  it("describes what a campaign gives in human units", () => {
    expect(describeAction({ type: "OrderPercentage", bps: 1250 })).toBe("12.5 % off the order");
    expect(describeAction({ type: "OrderFixedAmount", amount: { amount: 500, currency: "EUR" } })).toBe("5.00 EUR off the order");
  });
});

import { campaignDraftFromForm } from "./campaign";

describe("creation form", () => {
  const base = { promotion_id: "SUMMER", group_id: "g", priority: "0", "action.type": "OrderPercentage", "action.percent": "10" };
  it("Full creation: promotion, budget, metadata and codes from one form", () => {
    const r = campaignDraftFromForm({ ...base, title: "Summer sale", source: "hotels", labels: "", "budget.amount": "1000.00", "budget.currency": "EUR", "codes.count": "5", "codes.prefix": "SUM-", "codes.max_uses": "1" });
    expect(r).toMatchObject({
      ok: true,
      value: {
        promotion: { promotion_id: "SUMMER", action: { bps: 1000 } },
        metadata: { title: "Summer sale", source: "hotels", labels: {} },
        budget: { amount: 100000, currency: "EUR" },
        generate: { promotion_id: "SUMMER", count: 5, prefix: "SUM-", max_uses: 1, customer_id: null },
      },
    });
  });
  it("without title, budget or codes those steps are null", () => {
    expect(campaignDraftFromForm(base)).toMatchObject({ ok: true, value: { metadata: null, budget: null, generate: null } });
  });
  it("errors from every part are collected with their field names", () => {
    const r = campaignDraftFromForm({ ...base, "action.percent": "abc", "budget.amount": "x", "budget.currency": "EUR", "codes.count": "0" });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors.map((e) => e.field).sort()).toEqual(["action.percent", "budget.amount", "codes.count"]);
  });
});
