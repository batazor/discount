import { describe, expect, it } from "vitest";
import { claimsOf } from "../auth/oidc";
import { accessOf, navigation } from "./access";

const claims = (roles: string[]) => claimsOf({ sub: "u", realm_access: { roles } });

describe("spec admin-ui: navigation follows scopes", () => {
  it("Support account: support screens first, campaigns after, settings last", () => {
    const a = accessOf(claims(["discount:pricing", "discount:customers", "discount:catalog.read"]), true);
    expect(a).toEqual({ read: true, write: false, pricing: true, customers: true, admin: false });
    expect(navigation(a).map(([t, l]) => [t, l.map(([h]) => h)])).toEqual([
      ["Support", ["/find", "/explain", "/customers", "/redemptions"]],
      ["Campaigns", ["/campaigns", "/coupons"]],
      ["Settings", ["/settings/policy", "/settings/groups", "/settings/exchange-rates", "/settings/grant-rules"]],
    ]);
  });
  it("marketing leads with campaigns; a pricing-only token sees only its screens", () => {
    expect(navigation(accessOf(claims(["discount:catalog.write", "discount:catalog.read", "discount:pricing", "discount:customers"]), true))[0]![0]).toBe("Campaigns");
    expect(navigation(accessOf(claims(["discount:pricing"]), true))).toEqual([["Support", [["/find", "Find"], ["/explain", "Explain"], ["/redemptions", "Redemptions"]]]]);
  });
  it("without sign-in everything is offered; admin implies everything", () => {
    expect(accessOf(null, false)).toMatchObject({ read: true, write: true, pricing: true, customers: true });
    expect(accessOf(claims(["discount:admin"]), true)).toMatchObject({ read: true, write: true, pricing: true, customers: true, admin: true });
  });
});
