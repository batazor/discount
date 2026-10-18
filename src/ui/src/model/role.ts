// Which navigation and overview a viewer gets. Nothing is hidden by role.
export const ROLES = ["marketing", "support"] as const;
export type Role = (typeof ROLES)[number];

export function roleFrom(value: string | undefined): Role {
  return value === "support" ? "support" : "marketing";
}

export type NavGroup = [title: string, links: [href: string, label: string][]];

const CAMPAIGNS: NavGroup = ["Campaigns", [["/campaigns", "Campaigns"], ["/coupons", "Coupons"]]];
const SUPPORT: NavGroup = ["Support", [["/find", "Find"], ["/explain", "Explain"], ["/customers", "Customers"], ["/redemptions", "Redemptions"]]];
const SETTINGS: NavGroup = ["Settings", [["/settings/policy", "Pricing policy"], ["/settings/groups", "Stacking groups"], ["/settings/exchange-rates", "Exchange rates"], ["/settings/grant-rules", "Grant rules"]]];

export function navigation(role: Role): NavGroup[] {
  return role === "support" ? [SUPPORT, CAMPAIGNS, SETTINGS] : [CAMPAIGNS, SUPPORT, SETTINGS];
}

export const ROLE_HELP: Record<Role, string> = {
  marketing: "Campaign health first; create and run campaigns.",
  support: "Find a customer or an order first; explain what they got.",
};
