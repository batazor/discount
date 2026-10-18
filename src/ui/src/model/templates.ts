// Starting points for the wizard (design.md, decision 4): field values, nothing more.
import type { Fields } from "./fields";

export type Template = { id: string; title: string; description: string; fields: Fields; next?: { href: string; label: string } };

export const TEMPLATES: Template[] = [
  { id: "everyone", title: "Percent off for everyone", description: "A share off every order while it runs.", fields: { "action.type": "OrderPercentage", "action.percent": "10" } },
  { id: "welcome", title: "Welcome offer", description: "First order only, once per customer.", fields: { "action.type": "OrderPercentage", "action.percent": "15", "b.first_order": "on", "b.once": "on" } },
  { id: "code", title: "Promo code", description: "Only with a code; generates single-use codes.", fields: { "action.type": "OrderPercentage", "action.percent": "10", "b.needs_code": "on", "codes.count": "100", "codes.prefix": "PROMO-", "codes.max_uses": "1" } },
  { id: "sale", title: "Date-bound sale", description: "Runs between two dates, with a budget.", fields: { "action.type": "OrderPercentage", "action.percent": "20", "b.dates": "on", "budget.currency": "EUR" } },
  { id: "vip", title: "VIP allowlist", description: "Only customers on the allowlist.", fields: { "action.type": "OrderPercentage", "action.percent": "10", "b.allowlist": "on" }, next: { href: "?tab=audience", label: "add customers to the allowlist" } },
  { id: "winback", title: "Win-back after a cancellation", description: "A fixed amount with a code, once per customer; a grant rule issues the code on cancel.", fields: { "action.type": "OrderFixedAmount", "action.amount.amount": "5.00", "action.amount.currency": "EUR", "b.needs_code": "on", "b.once": "on" }, next: { href: "/settings/grant-rules/new", label: "create the grant rule for cancellations" } },
];

export function template(id: string | null | undefined): Template | null {
  return TEMPLATES.find((t) => t.id === id) ?? null;
}
