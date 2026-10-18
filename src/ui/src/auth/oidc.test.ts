import { describe, expect, it } from "vitest";
import { authorizeUrl, claimsOf, decodeJwt, defaultRole } from "./oidc";

const payload = { sub: "u1", preferred_username: "marketing", tenant_id: "hotels", realm_access: { roles: ["discount:catalog.write", "offline_access"] }, exp: 1760227200 };
const token = `eyJhbGciOiJSUzI1NiJ9.${Buffer.from(JSON.stringify(payload)).toString("base64url")}.sig`;

describe("oidc helpers", () => {
  it("decodes a JWT payload without verifying it", () => {
    expect(decodeJwt(token)).toEqual(payload);
    expect(decodeJwt("nope")).toBeNull();
  });
  it("reads name, tenant, discount scopes and expiry the way the service does", () => {
    expect(claimsOf(decodeJwt(token))).toEqual({ sub: "u1", name: "marketing", tenant: "hotels", scopes: ["discount:catalog.write"], exp: 1760227200 });
    expect(claimsOf({ scope: "discount:pricing openid" }).scopes).toEqual(["discount:pricing"]);
    expect(claimsOf(null).name).toBe("signed in");
  });
  it("role defaults from scopes", () => {
    expect(defaultRole(["discount:catalog.write"])).toBe("marketing");
    expect(defaultRole(["discount:admin"])).toBe("marketing");
    expect(defaultRole(["discount:pricing", "discount:customers"])).toBe("support");
  });
  it("builds the authorization URL with PKCE", () => {
    const u = new URL(authorizeUrl({ token_endpoint: "t", authorization_endpoint: "https://idp/auth" }, { clientId: "c", scopes: "openid" }, "http://localhost:4321/auth/callback", "ch", "st"));
    expect(Object.fromEntries(u.searchParams)).toEqual({ response_type: "code", client_id: "c", redirect_uri: "http://localhost:4321/auth/callback", scope: "openid", state: "st", code_challenge: "ch", code_challenge_method: "S256" });
  });
});
