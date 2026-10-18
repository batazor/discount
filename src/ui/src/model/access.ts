// What the signed-in token lets the console offer (spec: Navigation follows scopes, Actions
// follow scopes). The service remains the authority; this only decides what to show.
import type { Claims } from "../auth/oidc";

export type Access = { read: boolean; write: boolean; pricing: boolean; customers: boolean; admin: boolean };

export const ALL: Access = { read: true, write: true, pricing: true, customers: true, admin: false };

export function accessOf(user: Claims | null, authEnabled: boolean): Access {
  if (!authEnabled) return ALL;
  const s = new Set(user?.scopes ?? []);
  const admin = s.has("discount:admin");
  return {
    read: admin || s.has("discount:catalog.read"),
    write: admin || s.has("discount:catalog.write"),
    pricing: admin || s.has("discount:pricing"),
    customers: admin || s.has("discount:customers"),
    admin,
  };
}

export type NavGroup = [title: string, links: [href: string, label: string][]];

export function navigation(a: Access): NavGroup[] {
  const campaigns: NavGroup = ["Campaigns", [["/campaigns", "Campaigns"], ["/coupons", "Coupons"]]];
  const support: NavGroup = ["Support", [
    ...(a.customers || a.pricing ? [["/find", "Find"] as [string, string]] : []),
    ...(a.pricing ? [["/explain", "Explain"] as [string, string]] : []),
    ...(a.customers ? [["/customers", "Customers"] as [string, string]] : []),
    ...(a.pricing ? [["/redemptions", "Redemptions"] as [string, string]] : []),
  ]];
  const settings: NavGroup = ["Settings", [["/settings/policy", "Pricing policy"], ["/settings/groups", "Stacking groups"], ["/settings/exchange-rates", "Exchange rates"], ["/settings/grant-rules", "Grant rules"]]];
  const groups = a.write ? [campaigns, support, settings] : [support, campaigns, settings];
  return groups.filter(([title, links]) => links.length > 0 && (title === "Support" || a.read));
}
