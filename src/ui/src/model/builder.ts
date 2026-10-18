// The rule builder in plain words (design.md, decision 3): named conditions with their
// inputs, mapped to `rules.N.*` fields that the promotion mapping already understands.
import type { Fields } from "./fields";

export const CONDITIONS = [
  "dates",
  "min_total",
  "first_order",
  "once",
  "tags",
  "attr",
  "max_total",
  "needs_code",
  "allowlist",
] as const;
export type Condition = (typeof CONDITIONS)[number];

export const CONDITION_TEXT: Record<Condition, { label: string; description: string }> = {
  dates: { label: "Dates", description: "Runs from a start until an end; either may be open." },
  min_total: { label: "Minimum order total", description: "Only carts whose subtotal reaches this amount." },
  first_order: { label: "First order only", description: "Customers who have not completed an order yet." },
  once: { label: "Once per customer", description: "A customer gets it on one order only." },
  tags: { label: "Cart tags", description: "Only carts carrying at least one of these tags." },
  attr: { label: "Customer attribute", description: "Only customers whose attribute has one of these values." },
  max_total: { label: "Limit total uses", description: "Stops after this many orders in total." },
  needs_code: { label: "Needs a code", description: "Only with a valid coupon code for this campaign." },
  allowlist: { label: "Only allowlisted customers", description: "Only customers on the campaign's allowlist." },
};

const on = (f: Fields, key: string) => ["on", "true", "1"].includes((f[`b.${key}`] ?? "").trim().toLowerCase());

/** Builder fields → `rules.N.*` fields, in a fixed order. Existing `rules.*` are dropped. */
export function rulesFromBuilder(fields: Fields): Fields {
  const out: Fields = {};
  let n = 0;
  const rule = (type: string, extra: Record<string, string> = {}) => {
    out[`rules.${n}.type`] = type;
    for (const [k, v] of Object.entries(extra)) out[`rules.${n}.${k}`] = v;
    n++;
  };
  if (on(fields, "dates")) rule("DateWindow", { starts_at: fields["b.starts_at"] ?? "", ends_at: fields["b.ends_at"] ?? "" });
  if (on(fields, "min_total")) rule("OrderTotalRange", { "min.amount": fields["b.min_total.amount"] ?? "", "min.currency": fields["b.min_total.currency"] ?? "", "max.amount": "", "max.currency": "" });
  if (on(fields, "first_order")) rule("MaxCompletedOrders", { max: "0" });
  if (on(fields, "once")) rule("MaxUsesPerCustomer", { max: "1" });
  if (on(fields, "tags")) rule("TagsMatch", { all: "", any: fields["b.tags_any"] ?? "", none: "" });
  if (on(fields, "attr")) rule("CustomerAttributeIn", { key: fields["b.attr.key"] ?? "", values: fields["b.attr.values"] ?? "" });
  if (on(fields, "max_total")) rule("MaxUsesTotal", { max: fields["b.max_total.max"] ?? "" });
  if (on(fields, "needs_code")) rule("RequiresCoupon");
  if (on(fields, "allowlist")) rule("RequiresAllowlist");
  return out;
}

/** The fields with builder rules applied, when the form says it used the builder. */
export function applyBuilder(fields: Fields): Fields {
  if (fields.builder !== "1") return fields;
  const kept: Fields = {};
  for (const [k, v] of Object.entries(fields)) if (!k.startsWith("rules.")) kept[k] = v;
  return { ...kept, ...rulesFromBuilder(fields) };
}
