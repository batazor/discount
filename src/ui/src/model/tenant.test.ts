import { describe, expect, it } from "vitest";
import { claimsOf } from "../auth/oidc";
import { tenantLabel } from "./tenant";

describe("spec admin-ui: tenant visible", () => {
  it("Signed in to hotels", () => {
    const claims = claimsOf({ sub: "u", preferred_username: "marketing", tenant_id: "hotels", realm_access: { roles: ["discount:catalog.write"] } });
    expect(tenantLabel(claims, true)).toEqual({ name: "hotels", fallback: false, note: null });
  });
  it("default when the token names no tenant, local without sign-in", () => {
    expect(tenantLabel(claimsOf({ sub: "a", realm_access: { roles: ["discount:admin"] } }), true)).toMatchObject({ name: "default", fallback: true });
    expect(tenantLabel(null, false)).toMatchObject({ name: "local", fallback: false });
  });
});
