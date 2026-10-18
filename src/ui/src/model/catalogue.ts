// Form ↔ body for the rest of the catalogue: policy, groups, coupons, grant rules, rates,
// coupon generation and grants.
import type {
  Coupon,
  ExchangeRate,
  GenerateCouponsRequest,
  GrantRequest,
  GrantRule,
  PricingPolicy,
  StackingGroup,
} from "../api/types";
import { Form, flatten, type Fields, type Parsed } from "./fields";
import { ROUNDING_POLICIES } from "./promotion";
import { toDatetimeLocal } from "./time";

export const STACKING_POLICIES = ["Sequential", "Additive"] as const;
export const STACKING_MODES = ["Stackable", "FirstMatch"] as const;
export const GRANT_TRIGGERS = ["cancel", "review"] as const;

export function policyFromForm(fields: Fields): Parsed<PricingPolicy> {
  const f = new Form(fields);
  return f.finish({
    rounding: { type: f.enum("rounding", ROUNDING_POLICIES) },
    stacking: { type: f.enum("stacking", STACKING_POLICIES) },
    order_cap: {
      amount: f.optMoney("order_cap.amount"),
      bps: f.optInt("order_cap.bps", { min: 0, max: 10000 }),
    },
  });
}

export function policyToFields(p: PricingPolicy): Fields {
  return {
    rounding: p.rounding.type,
    stacking: p.stacking.type,
    ...flatten(p.order_cap.amount, "order_cap.amount"),
    "order_cap.bps": p.order_cap.bps === null ? "" : String(p.order_cap.bps),
  };
}

export function groupFromForm(fields: Fields): Parsed<StackingGroup> {
  const f = new Form(fields);
  return f.finish({
    group_id: f.str("group_id"),
    priority: f.int("priority"),
    mode: { type: f.enum("mode", STACKING_MODES) },
  });
}

export function groupToFields(g: StackingGroup): Fields {
  return { group_id: g.group_id, priority: String(g.priority), mode: g.mode.type };
}

/** `uses` is derived by the service and ignored on PUT: shown read-only, sent as 0. */
export function couponFromForm(fields: Fields): Parsed<Coupon> {
  const f = new Form(fields);
  return f.finish({
    code: f.str("code"),
    promotion_id: f.str("promotion_id"),
    starts_at: f.timestamp("starts_at"),
    ends_at: f.timestamp("ends_at"),
    customer_id: f.optStr("customer_id"),
    max_uses: f.optInt("max_uses", { min: 0 }),
    uses: 0,
  });
}

export function couponToFields(c: Coupon): Fields {
  return {
    code: c.code,
    promotion_id: c.promotion_id,
    starts_at: toDatetimeLocal(c.starts_at),
    ends_at: toDatetimeLocal(c.ends_at),
    customer_id: c.customer_id ?? "",
    max_uses: c.max_uses === null ? "" : String(c.max_uses),
  };
}

export function grantRuleFromForm(fields: Fields): Parsed<GrantRule> {
  const f = new Form(fields);
  return f.finish({
    rule_id: f.str("rule_id"),
    trigger: f.enum("trigger", GRANT_TRIGGERS),
    promotion_id: f.str("promotion_id"),
    priority: f.int("priority"),
    validity_seconds: f.int("validity_seconds", { min: 1 }),
    max_uses: f.optInt("max_uses", { min: 0 }),
  });
}

export function grantRuleToFields(r: GrantRule): Fields {
  return {
    rule_id: r.rule_id,
    trigger: r.trigger,
    promotion_id: r.promotion_id,
    priority: String(r.priority),
    validity_seconds: String(r.validity_seconds),
    max_uses: r.max_uses === null ? "" : String(r.max_uses),
  };
}

export function exchangeRateFromForm(fields: Fields): Parsed<ExchangeRate> {
  const f = new Form(fields);
  return f.finish({
    from: f.str("from"),
    to: f.str("to"),
    numerator: f.int("numerator", { min: 1 }),
    denominator: f.int("denominator", { min: 1 }),
  });
}

export function exchangeRateToFields(r: ExchangeRate): Fields {
  return { from: r.from, to: r.to, numerator: String(r.numerator), denominator: String(r.denominator) };
}

export function generateRequestFromForm(fields: Fields): Parsed<GenerateCouponsRequest> {
  const f = new Form(fields);
  return f.finish({
    promotion_id: f.str("promotion_id"),
    count: f.int("count", { min: 1, max: 10000 }),
    prefix: f.raw("prefix"),
    starts_at: f.timestamp("starts_at"),
    ends_at: f.timestamp("ends_at"),
    max_uses: f.optInt("max_uses", { min: 0 }),
    customer_id: f.optStr("customer_id"),
  });
}

export function grantRequestFromForm(fields: Fields): Parsed<GrantRequest> {
  const f = new Form(fields);
  return f.finish({
    trigger: f.enum("trigger", GRANT_TRIGGERS),
    customer_id: f.str("customer_id"),
    order_id: f.str("order_id"),
  });
}
