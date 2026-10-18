import type { APIRoute } from "astro";
import { authEnabled, OIDC } from "../../auth/config";
import { discover, exchangeCode } from "../../auth/oidc";
import { SESSION_KEY } from "../../auth/session";
import { sessionFrom } from "../../middleware";

// Authorization-code flow return leg (DISCOUNT_UI_OIDC_FLOW=code).
export const GET: APIRoute = async ({ url, session, redirect }) => {
  if (!authEnabled) return redirect("/", 303);
  const pending = (await session?.get("pkce")) as { verifier: string; state: string; back: string } | undefined;
  await session?.delete("pkce");
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  if (!pending || !code || state !== pending.state) return new Response("Sign-in did not complete: state mismatch or no code.", { status: 400 });
  const t = await exchangeCode(await discover(OIDC.issuer), OIDC, code, `${url.origin}/auth/callback`, pending.verifier);
  if ("error" in t) return new Response(`Sign-in failed: ${t.error_description ?? t.error}`, { status: 400 });
  const s = sessionFrom(t);
  await session?.set(SESSION_KEY, s);
  return redirect(pending.back || "/", 303);
};
