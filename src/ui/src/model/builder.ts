// The rule builder in plain words (design: admin-ui-campaign-builder, admin-ui-templates-rules):
// named conditions with their inputs, mapped to `rules.N.*` fields that the promotion mapping
// already understands, and back from stored rules to conditions where they fit.
import type { Rule } from "../api/types";
import { moneyToFields, type FieldError, type Fields } from "./fields";
import { ruleToFields } from "./promotion";
import { parseTimestamp, toDatetimeLocal } from "./time";
import { parseAmount } from "./units";

export const CONDITIONS = [
  "dates",
  "min_total",
  "max_order",
  "first_order",
  "once",
  "tags",
  "cart_attr",
  "cart_date",
  "attr",
  "attr_range",
  "max_total",
  "needs_code",
  "allowlist",
] as const;
export type Condition = (typeof CONDITIONS)[number];

export const CONDITION_TEXT: Record<Condition, { label: string; description: string }> = {
  dates: { label: "Dates", description: "Runs from a start until an end; either may be open." },
  min_total: { label: "Minimum order total", description: "Only carts whose subtotal reaches this amount." },
  max_order: { label: "Maximum order total", description: "Only carts whose subtotal stays within this amount." },
  first_order: { label: "First order only", description: "Customers who have not completed an order yet." },
  once: { label: "Once per customer", description: "A customer gets it on one order only." },
  tags: { label: "Cart tags", description: "Only carts carrying at least one of these tags." },
  cart_attr: { label: "Cart attribute", description: "Only carts whose attribute has one of these values." },
  cart_date: { label: "Cart date attribute", description: "Only carts whose date attribute (as Unix seconds) falls in a window, e.g. check-in." },
  attr: { label: "Customer attribute", description: "Only customers whose attribute has one of these values." },
  attr_range: { label: "Customer attribute range", description: "Only customers whose numeric attribute is within bounds, e.g. app orders at most 0." },
  max_total: { label: "Limit total uses", description: "Stops after this many orders in total." },
  needs_code: { label: "Needs a code", description: "Only with a valid coupon code for this campaign." },
  allowlist: { label: "Only allowlisted customers", description: "Only customers on the campaign's allowlist." },
};

const on = (f: Fields, key: string) => ["on", "true", "1"].includes((f[`b.${key}`] ?? "").trim().toLowerCase());

/** Builder fields → `rules.N.*` fields, in a fixed order, numbered from `start`. */
export function rulesFromBuilder(fields: Fields, start = 0): Fields {
  const out: Fields = {};
  let n = start;
  const rule = (type: string, extra: Record<string, string> = {}) => {
    out[`rules.${n}.type`] = type;
    for (const [k, v] of Object.entries(extra)) out[`rules.${n}.${k}`] = v;
    n++;
  };
  if (on(fields, "dates")) rule("DateWindow", { starts_at: fields["b.starts_at"] ?? "", ends_at: fields["b.ends_at"] ?? "" });
  if (on(fields, "min_total") || on(fields, "max_order")) {
    rule("OrderTotalRange", {
      "min.amount": on(fields, "min_total") ? (fields["b.min_total.amount"] ?? "") : "",
      "min.currency": on(fields, "min_total") ? (fields["b.min_total.currency"] ?? "") : "",
      "max.amount": on(fields, "max_order") ? (fields["b.max_order.amount"] ?? "") : "",
      "max.currency": on(fields, "max_order") ? (fields["b.max_order.currency"] ?? "") : "",
    });
  }
  if (on(fields, "first_order")) rule("MaxCompletedOrders", { max: "0" });
  if (on(fields, "once")) rule("MaxUsesPerCustomer", { max: "1" });
  if (on(fields, "tags")) rule("TagsMatch", { all: "", any: fields["b.tags_any"] ?? "", none: "" });
  if (on(fields, "cart_attr")) rule("AttributeIn", { key: fields["b.cart_attr.key"] ?? "", values: fields["b.cart_attr.values"] ?? "" });
  if (on(fields, "cart_date")) rule("AttributeDateWindow", { key: fields["b.cart_date.key"] ?? "", starts_at: fields["b.cart_date.starts_at"] ?? "", ends_at: fields["b.cart_date.ends_at"] ?? "" });
  if (on(fields, "attr")) rule("CustomerAttributeIn", { key: fields["b.attr.key"] ?? "", values: fields["b.attr.values"] ?? "" });
  if (on(fields, "attr_range")) rule("CustomerAttributeRange", { key: fields["b.attr_range.key"] ?? "", min: fields["b.attr_range.min"] ?? "", max: fields["b.attr_range.max"] ?? "" });
  if (on(fields, "max_total")) rule("MaxUsesTotal", { max: fields["b.max_total.max"] ?? "" });
  if (on(fields, "needs_code")) rule("RequiresCoupon");
  if (on(fields, "allowlist")) rule("RequiresAllowlist");
  return out;
}

/** Indices of advanced (raw) rule rows present in the fields, in order. */
function rawRows(fields: Fields): number[] {
  const idx = new Set<number>();
  for (const k of Object.keys(fields)) {
    const m = /^rules\.(\d+)\./.exec(k);
    if (m && (fields[`rules.${m[1]}.type`] ?? "") !== "") idx.add(Number(m[1]));
  }
  return [...idx].sort((a, b) => a - b);
}

/**
 * The fields with builder rules applied when the form says it used the builder: builder
 * rules first, then the advanced `rules.N.*` rows renumbered after them.
 */
export function applyBuilder(fields: Fields): Fields {
  if (fields.builder !== "1") return fields;
  const kept: Fields = {};
  for (const [k, v] of Object.entries(fields)) if (!k.startsWith("rules.")) kept[k] = v;
  const built = rulesFromBuilder(fields);
  let n = Object.keys(built).filter((k) => k.endsWith(".type")).length;
  for (const i of rawRows(fields)) {
    for (const [k, v] of Object.entries(fields)) {
      if (k.startsWith(`rules.${i}.`)) built[`rules.${n}.${k.slice(`rules.${i}.`.length)}`] = v;
    }
    n++;
  }
  return { ...kept, ...built };
}

/** Stored rules → builder fields where a condition expresses them; the rest are leftovers. */
export function builderFromRules(rules: Rule[]): { fields: Fields; leftover: Rule[] } {
  const f: Fields = {};
  const leftover: Rule[] = [];
  const seen = new Set<string>();
  const once = (key: string) => (seen.has(key) ? false : (seen.add(key), true));
  for (const r of rules) {
    switch (r.type) {
      case "DateWindow":
        if (once("dates")) Object.assign(f, { "b.dates": "on", "b.starts_at": toDatetimeLocal(r.starts_at), "b.ends_at": toDatetimeLocal(r.ends_at) });
        else leftover.push(r);
        break;
      case "OrderTotalRange":
        if (once("total")) {
          if (r.min) Object.assign(f, { "b.min_total": "on" }, moneyToFields(r.min, "b.min_total"));
          if (r.max) Object.assign(f, { "b.max_order": "on" }, moneyToFields(r.max, "b.max_order"));
          if (!r.min && !r.max) leftover.push(r);
        } else leftover.push(r);
        break;
      case "MaxCompletedOrders":
        if (r.max === 0 && once("first")) f["b.first_order"] = "on";
        else leftover.push(r);
        break;
      case "MaxUsesPerCustomer":
        if (r.max === 1 && once("once")) f["b.once"] = "on";
        else leftover.push(r);
        break;
      case "TagsMatch":
        if (r.all.length === 0 && r.none.length === 0 && r.any.length > 0 && once("tags")) Object.assign(f, { "b.tags": "on", "b.tags_any": r.any.join(", ") });
        else leftover.push(r);
        break;
      case "AttributeIn":
        if (once("cart_attr")) Object.assign(f, { "b.cart_attr": "on", "b.cart_attr.key": r.key, "b.cart_attr.values": r.values.join(", ") });
        else leftover.push(r);
        break;
      case "AttributeDateWindow":
        if (once("cart_date")) Object.assign(f, { "b.cart_date": "on", "b.cart_date.key": r.key, "b.cart_date.starts_at": toDatetimeLocal(r.starts_at), "b.cart_date.ends_at": toDatetimeLocal(r.ends_at) });
        else leftover.push(r);
        break;
      case "CustomerAttributeIn":
        if (once("attr")) Object.assign(f, { "b.attr": "on", "b.attr.key": r.key, "b.attr.values": r.values.join(", ") });
        else leftover.push(r);
        break;
      case "CustomerAttributeRange":
        if (once("attr_range")) Object.assign(f, { "b.attr_range": "on", "b.attr_range.key": r.key, "b.attr_range.min": r.min === null ? "" : String(r.min), "b.attr_range.max": r.max === null ? "" : String(r.max) });
        else leftover.push(r);
        break;
      case "MaxUsesTotal":
        if (once("max_total")) Object.assign(f, { "b.max_total": "on", "b.max_total.max": String(r.max) });
        else leftover.push(r);
        break;
      case "RequiresCoupon":
        if (once("needs_code")) f["b.needs_code"] = "on";
        break;
      case "RequiresAllowlist":
        if (once("allowlist")) f["b.allowlist"] = "on";
        break;
    }
  }
  return { fields: f, leftover };
}

/** Leftover rules as advanced `rules.N.*` fields, numbered from `start`. */
export function leftoverFields(leftover: Rule[], start = 0): Fields {
  const out: Fields = {};
  leftover.forEach((r, i) => Object.assign(out, ruleToFields(r, `rules.${start + i}`)));
  return out;
}

/** Sanity checks on builder fields: mistakes the engine would accept as always-false rules. */
export function builderErrors(fields: Fields): FieldError[] {
  const errors: FieldError[] = [];
  const ts = (k: string) => parseTimestamp(fields[k] ?? "");
  if (on(fields, "dates")) {
    const s = ts("b.starts_at"), e = ts("b.ends_at");
    if (typeof s === "number" && typeof e === "number" && e <= s) errors.push({ field: "b.ends_at", message: "ends before it starts" });
  }
  if (on(fields, "cart_date")) {
    const s = ts("b.cart_date.starts_at"), e = ts("b.cart_date.ends_at");
    if (typeof s === "number" && typeof e === "number" && e <= s) errors.push({ field: "b.cart_date.ends_at", message: "ends before it starts" });
  }
  if (on(fields, "min_total") && on(fields, "max_order")) {
    const cur = (fields["b.min_total.currency"] ?? "").trim();
    const min = parseAmount(fields["b.min_total.amount"] ?? "", cur);
    const max = parseAmount(fields["b.max_order.amount"] ?? "", (fields["b.max_order.currency"] ?? "").trim());
    if (min !== undefined && max !== undefined && cur === (fields["b.max_order.currency"] ?? "").trim() && min > max) errors.push({ field: "b.max_order.amount", message: "below the minimum" });
  }
  if (on(fields, "attr_range")) {
    const min = Number(fields["b.attr_range.min"] ?? ""), max = Number(fields["b.attr_range.max"] ?? "");
    const hasMin = (fields["b.attr_range.min"] ?? "").trim() !== "", hasMax = (fields["b.attr_range.max"] ?? "").trim() !== "";
    if (!hasMin && !hasMax) errors.push({ field: "b.attr_range.min", message: "give at least one bound" });
    else if (hasMin && hasMax && min > max) errors.push({ field: "b.attr_range.max", message: "below the minimum" });
  }
  return errors;
}
