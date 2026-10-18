// Console-side OIDC settings. Without an issuer the console runs without sign-in, as it
// does against a service with DISCOUNT_AUTH=off.
export const OIDC = {
  issuer: (process.env.DISCOUNT_UI_OIDC_ISSUER ?? "").trim().replace(/\/$/, ""),
  clientId: process.env.DISCOUNT_UI_OIDC_CLIENT_ID ?? "discount-console",
  clientSecret: process.env.DISCOUNT_UI_OIDC_CLIENT_SECRET || undefined,
  scopes: process.env.DISCOUNT_UI_OIDC_SCOPES ?? "openid profile",
  flow: process.env.DISCOUNT_UI_OIDC_FLOW === "code" ? "code" : "password",
  /** `user:password,user:password` shown on the sign-in page of a demo stack. */
  demoUsers: (process.env.DISCOUNT_UI_DEMO_USERS ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => { const i = s.indexOf(":"); return i < 0 ? { user: s, password: "" } : { user: s.slice(0, i), password: s.slice(i + 1) }; }),
} as const;

export const authEnabled = OIDC.issuer !== "";

/** Paths that never need a session. */
export function isPublicPath(path: string): boolean {
  return path === "/login" || path === "/logout" || path.startsWith("/auth/") || path === "/healthz" || path === "/api-docs" || path === "/openapi.yaml" || path.startsWith("/_astro/");
}
