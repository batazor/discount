// What the server-side session holds for a signed-in operator (design: tokens never reach
// the browser) and the request-scoped bearer the API client reads.
import { AsyncLocalStorage } from "node:async_hooks";
import type { Claims } from "./oidc";

export type AuthSession = {
  access_token: string;
  refresh_token: string | null;
  /** Unix seconds. */
  expires_at: number;
  claims: Claims;
};

export const SESSION_KEY = "auth";

export const bearer = new AsyncLocalStorage<{ token: string | null }>();

export function currentToken(): string | null {
  return bearer.getStore()?.token ?? null;
}
