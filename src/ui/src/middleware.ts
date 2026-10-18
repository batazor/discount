// Sign-in gate (spec: Signing in): with an issuer configured, every non-public page needs a
// session; a token about to expire is refreshed first; the request then runs with the
// bearer in scope so the API client attaches it.
import { defineMiddleware } from "astro:middleware";
import { authEnabled, isPublicPath, OIDC } from "./auth/config";
import { discover, refreshGrant, claimsOf, decodeJwt } from "./auth/oidc";
import { bearer, SESSION_KEY, type AuthSession } from "./auth/session";

export const onRequest = defineMiddleware(async (context, next) => {
  if (!authEnabled) return bearer.run({ token: null }, () => next());
  const path = context.url.pathname;
  let auth = (await context.session?.get(SESSION_KEY)) as AuthSession | undefined;
  const now = Math.floor(Date.now() / 1000);
  if (auth && auth.expires_at - now < 60) {
    auth = auth.refresh_token ? await refresh(auth.refresh_token) : undefined;
    if (auth) await context.session?.set(SESSION_KEY, auth);
    else await context.session?.delete(SESSION_KEY);
  }
  context.locals.user = auth?.claims ?? null;
  if (!auth && !isPublicPath(path)) {
    const back = encodeURIComponent(path + context.url.search);
    return context.redirect(`/login?back=${back}`, 303);
  }
  return bearer.run({ token: auth?.access_token ?? null }, () => next());
});

async function refresh(refreshToken: string): Promise<AuthSession | undefined> {
  try {
    const d = await discover(OIDC.issuer);
    const t = await refreshGrant(d, OIDC, refreshToken);
    if ("error" in t) return undefined;
    return sessionFrom(t);
  } catch {
    return undefined;
  }
}

export function sessionFrom(t: { access_token: string; refresh_token?: string; expires_in?: number }): AuthSession {
  const claims = claimsOf(decodeJwt(t.access_token));
  const now = Math.floor(Date.now() / 1000);
  return { access_token: t.access_token, refresh_token: t.refresh_token ?? null, expires_at: claims.exp ?? now + (t.expires_in ?? 300), claims };
}
