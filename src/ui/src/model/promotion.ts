// Promotion form ↔ contract body. Every Rule and Action variant of api/openapi.yaml is
// offered as a choice; the submitted object is the tagged ubiquitous-language shape.
import type { Action, Complete, Promotion, Rule, RoundingPolicy } from "../api/types";
import { Form, moneyToFields, type Fields, type Parsed } from "./fields";
import { formatPercentInput } from "./units";
import { toDatetimeLocal } from "./time";

export const RULE_TYPES = [
  "DateWindow",
  "OrderTotalRange",
  "TagsMatch",
  "AttributeIn",
  "AttributeDateWindow",
  "MaxCompletedOrders",
  "MaxUsesPerCustomer",
  "MaxUsesTotal",
  "RequiresCoupon",
  "CustomerAttributeIn",
  "CustomerAttributeRange",
  "RequiresAllowlist",
] as const satisfies readonly Rule["type"][];
export type RuleType = (typeof RULE_TYPES)[number];

export const ACTION_TYPES = ["OrderPercentage", "OrderFixedAmount"] as const satisfies readonly Action["type"][];
export const ROUNDING_POLICIES = ["HalfUp", "HalfEven", "Down"] as const satisfies readonly RoundingPolicy["type"][];

// The contract is the source of these lists (ADR 10): a variant added to api/openapi.yaml
// that the form does not offer fails `astro check` here, naming the variant.
export const RULE_TYPES_COMPLETE: Complete<Rule["type"], typeof RULE_TYPES> = true;
export const ACTION_TYPES_COMPLETE: Complete<Action["type"], typeof ACTION_TYPES> = true;
export const ROUNDING_POLICIES_COMPLETE: Complete<RoundingPolicy["type"], typeof ROUNDING_POLICIES> = true;

export function ruleFromForm(f: Form, p: string): Rule {
  const type = f.enum(`${p}.type`, RULE_TYPES);
  switch (type) {
    case "DateWindow":
      return { type, starts_at: f.timestamp(`${p}.starts_at`), ends_at: f.timestamp(`${p}.ends_at`) };
    case "OrderTotalRange":
      return { type, min: f.optMoney(`${p}.min`), max: f.optMoney(`${p}.max`) };
    case "TagsMatch":
      return { type, all: f.list(`${p}.all`), any: f.list(`${p}.any`), none: f.list(`${p}.none`) };
    case "AttributeIn":
      return { type, key: f.str(`${p}.key`), values: f.list(`${p}.values`) };
    case "AttributeDateWindow":
      return {
        type,
        key: f.str(`${p}.key`),
        starts_at: f.timestamp(`${p}.starts_at`),
        ends_at: f.timestamp(`${p}.ends_at`),
      };
    case "MaxCompletedOrders":
    case "MaxUsesPerCustomer":
    case "MaxUsesTotal":
      return { type, max: f.int(`${p}.max`, { min: 0 }) };
    case "RequiresCoupon":
    case "RequiresAllowlist":
      return { type };
    case "CustomerAttributeIn":
      return { type, key: f.str(`${p}.key`), values: f.list(`${p}.values`) };
    case "CustomerAttributeRange":
      return { type, key: f.str(`${p}.key`), min: f.optInt(`${p}.min`), max: f.optInt(`${p}.max`) };
  }
}

export function ruleToFields(r: Rule, p: string): Fields {
  const out: Fields = { [`${p}.type`]: r.type };
  switch (r.type) {
    case "DateWindow":
      out[`${p}.starts_at`] = toDatetimeLocal(r.starts_at);
      out[`${p}.ends_at`] = toDatetimeLocal(r.ends_at);
      break;
    case "OrderTotalRange":
      Object.assign(out, moneyToFields(r.min, `${p}.min`), moneyToFields(r.max, `${p}.max`));
      break;
    case "TagsMatch":
      out[`${p}.all`] = r.all.join(", ");
      out[`${p}.any`] = r.any.join(", ");
      out[`${p}.none`] = r.none.join(", ");
      break;
    case "AttributeIn":
      out[`${p}.key`] = r.key;
      out[`${p}.values`] = r.values.join(", ");
      break;
    case "AttributeDateWindow":
      out[`${p}.key`] = r.key;
      out[`${p}.starts_at`] = toDatetimeLocal(r.starts_at);
      out[`${p}.ends_at`] = toDatetimeLocal(r.ends_at);
      break;
    case "MaxCompletedOrders":
    case "MaxUsesPerCustomer":
    case "MaxUsesTotal":
      out[`${p}.max`] = String(r.max);
      break;
    case "RequiresCoupon":
    case "RequiresAllowlist":
      break;
    case "CustomerAttributeIn":
      out[`${p}.key`] = r.key;
      out[`${p}.values`] = r.values.join(", ");
      break;
    case "CustomerAttributeRange":
      out[`${p}.key`] = r.key;
      out[`${p}.min`] = r.min === null ? "" : String(r.min);
      out[`${p}.max`] = r.max === null ? "" : String(r.max);
      break;
  }
  return out;
}

export function actionFromForm(f: Form, p: string): Action {
  const type = f.enum(`${p}.type`, ACTION_TYPES);
  if (type === "OrderPercentage") return { type, bps: f.percent(`${p}.percent`) };
  return { type, amount: f.money(`${p}.amount`) };
}

export function actionToFields(a: Action, p: string): Fields {
  if (a.type === "OrderPercentage") return { [`${p}.type`]: a.type, [`${p}.percent`]: formatPercentInput(a.bps) };
  return { [`${p}.type`]: a.type, ...moneyToFields(a.amount, `${p}.amount`) };
}

export function roundingFromForm(f: Form, key: string): RoundingPolicy | null {
  const t = f.optEnum(key, ROUNDING_POLICIES);
  return t === null ? null : { type: t };
}

/**
 * The PUT body. `remaining_budget` and `uses_total` are derived by the service and ignored
 * on PUT (contract), so the form never sends what it shows: both go out as their zero.
 */
export function promotionFromForm(fields: Fields): Parsed<Promotion> {
  const f = new Form(fields);
  const promotion: Promotion = {
    promotion_id: f.str("promotion_id"),
    group_id: f.str("group_id"),
    priority: f.int("priority"),
    exclusive: f.bool("exclusive"),
    cap: f.optMoney("cap"),
    remaining_budget: null,
    uses_total: 0,
    rounding: roundingFromForm(f, "rounding"),
    rules: f.rows("rules", "type").map((i) => ruleFromForm(f, `rules.${i}`)),
    action: actionFromForm(f, "action"),
  };
  return f.finish(promotion);
}

export function promotionToFields(p: Promotion): Fields {
  const out: Fields = {
    promotion_id: p.promotion_id,
    group_id: p.group_id,
    priority: String(p.priority),
    exclusive: p.exclusive ? "on" : "",
    rounding: p.rounding?.type ?? "",
    ...moneyToFields(p.cap, "cap"),
    ...actionToFields(p.action, "action"),
  };
  p.rules.forEach((r, i) => Object.assign(out, ruleToFields(r, `rules.${i}`)));
  return out;
}
