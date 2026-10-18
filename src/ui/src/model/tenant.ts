// Which tenant the console works in (spec: Tenant visible). The service decides it from the
// token; the console only says it out loud.
import type { Claims } from "../auth/oidc";

export type TenantLabel = { name: string; fallback: boolean; note: string | null };

export function tenantLabel(user: Claims | null, authEnabled: boolean): TenantLabel {
  if (!authEnabled) return { name: "local", fallback: false, note: "No sign-in: the service's local catalogue." };
  if (user?.tenant) return { name: user.tenant, fallback: false, note: null };
  return { name: "default", fallback: true, note: "Your account names no tenant, so the service uses its default one. The console works in one tenant; switching is not offered." };
}
