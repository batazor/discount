import type { APIRoute } from "astro";
import { authEnabled, OIDC } from "../auth/config";
import { discover, revoke } from "../auth/oidc";
import { SESSION_KEY, type AuthSession } from "../auth/session";

// Sign-out: the session goes, and the refresh token is revoked when the issuer offers it.
export const POST: APIRoute = async ({ session, redirect }) => {
  const auth = (await session?.get(SESSION_KEY)) as AuthSession | undefined;
  await session?.delete(SESSION_KEY);
  if (authEnabled && auth?.refresh_token) {
    try { await revoke(await discover(OIDC.issuer), OIDC, auth.refresh_token); } catch { /* the session is gone either way */ }
  }
  return redirect(authEnabled ? "/login" : "/", 303);
};
export const GET: APIRoute = ({ redirect }) => redirect("/", 303);
