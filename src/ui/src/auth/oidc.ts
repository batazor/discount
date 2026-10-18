// Talking to the issuer: discovery, the token endpoint (password, code, refresh), revocation,
// and reading a JWT payload for display. Pure functions take their inputs; the ones that
// call the network are thin. The console verifies no signature: the service does.
import { createHash, randomBytes } from "node:crypto";

export type Discovery = {
  authorization_endpoint?: string;
  token_endpoint: string;
  end_session_endpoint?: string;
  revocation_endpoint?: string;
};

export type TokenSet = { access_token: string; refresh_token?: string; expires_in?: number; id_token?: string; token_type?: string };
export type TokenError = { error: string; error_description?: string };

export type Claims = {
  sub: string;
  name: string;
  tenant: string | null;
  scopes: string[];
  exp: number | null;
};

const cache = new Map<string, Promise<Discovery>>();

export function discover(issuer: string, fetchImpl: typeof fetch = fetch): Promise<Discovery> {
  let p = cache.get(issuer);
  if (!p) {
    p = fetchImpl(`${issuer}/.well-known/openid-configuration`).then(async (r) => {
      if (!r.ok) throw new Error(`discovery at ${issuer} answered ${r.status}`);
      return (await r.json()) as Discovery;
    });
    p.catch(() => cache.delete(issuer));
    cache.set(issuer, p);
  }
  return p;
}

async function tokenRequest(endpoint: string, params: Record<string, string>, fetchImpl: typeof fetch = fetch): Promise<TokenSet | TokenError> {
  const r = await fetchImpl(endpoint, { method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body: new URLSearchParams(params) });
  const body = (await r.json().catch(() => ({ error: "invalid_response", error_description: `token endpoint answered ${r.status}` }))) as TokenSet | TokenError;
  return r.ok ? body : ("error" in body ? body : { error: "invalid_response", error_description: `token endpoint answered ${r.status}` });
}

export type ClientAuth = { clientId: string; clientSecret?: string; scopes: string };

export function passwordGrant(d: Discovery, c: ClientAuth, username: string, password: string, fetchImpl?: typeof fetch) {
  return tokenRequest(d.token_endpoint, { grant_type: "password", client_id: c.clientId, ...(c.clientSecret ? { client_secret: c.clientSecret } : {}), username, password, scope: c.scopes }, fetchImpl);
}

export function refreshGrant(d: Discovery, c: ClientAuth, refreshToken: string, fetchImpl?: typeof fetch) {
  return tokenRequest(d.token_endpoint, { grant_type: "refresh_token", client_id: c.clientId, ...(c.clientSecret ? { client_secret: c.clientSecret } : {}), refresh_token: refreshToken }, fetchImpl);
}

export function exchangeCode(d: Discovery, c: ClientAuth, code: string, redirectUri: string, verifier: string, fetchImpl?: typeof fetch) {
  return tokenRequest(d.token_endpoint, { grant_type: "authorization_code", client_id: c.clientId, ...(c.clientSecret ? { client_secret: c.clientSecret } : {}), code, redirect_uri: redirectUri, code_verifier: verifier }, fetchImpl);
}

export async function revoke(d: Discovery, c: ClientAuth, token: string, fetchImpl: typeof fetch = fetch): Promise<void> {
  if (!d.revocation_endpoint) return;
  await fetchImpl(d.revocation_endpoint, { method: "POST", headers: { "content-type": "application/x-www-form-urlencoded" }, body: new URLSearchParams({ client_id: c.clientId, ...(c.clientSecret ? { client_secret: c.clientSecret } : {}), token, token_type_hint: "refresh_token" }) }).catch(() => undefined);
}

/** PKCE pair and state for the authorization-code flow. */
export function pkce(): { verifier: string; challenge: string; state: string } {
  const verifier = randomBytes(32).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  return { verifier, challenge, state: randomBytes(16).toString("base64url") };
}

export function authorizeUrl(d: Discovery, c: ClientAuth, redirectUri: string, challenge: string, state: string): string {
  if (!d.authorization_endpoint) throw new Error("issuer has no authorization endpoint");
  const u = new URL(d.authorization_endpoint);
  u.search = new URLSearchParams({ response_type: "code", client_id: c.clientId, redirect_uri: redirectUri, scope: c.scopes, state, code_challenge: challenge, code_challenge_method: "S256" }).toString();
  return u.toString();
}

/** The payload of a JWT, for display. No verification. */
export function decodeJwt(token: string): Record<string, unknown> | null {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  try {
    return JSON.parse(Buffer.from(parts[1]!, "base64url").toString("utf8")) as Record<string, unknown>;
  } catch {
    return null;
  }
}

/** Name, tenant, scopes and expiry from a token payload, the way the service reads them. */
export function claimsOf(payload: Record<string, unknown> | null, tenantClaim = "tenant_id"): Claims {
  const p = payload ?? {};
  const realm = (p.realm_access as { roles?: unknown } | undefined)?.roles;
  const scopes = Array.isArray(realm) ? realm.map(String) : typeof p.scope === "string" ? p.scope.split(" ").filter(Boolean) : [];
  const name = [p.name, p.preferred_username, p.email, p.sub].find((v) => typeof v === "string" && v) as string | undefined;
  return {
    sub: typeof p.sub === "string" ? p.sub : "",
    name: name ?? "signed in",
    tenant: typeof p[tenantClaim] === "string" ? (p[tenantClaim] as string) : null,
    scopes: scopes.filter((s) => s.startsWith("discount:")),
    exp: typeof p.exp === "number" ? p.exp : null,
  };
}
