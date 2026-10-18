import type { APIRoute } from "astro";

// The role switch (design.md, decision 4): a cookie set by a plain link, no scripts needed.
export const ROLES = ["marketing", "support"] as const;
export type Role = (typeof ROLES)[number];

export const GET: APIRoute = ({ url, cookies, redirect }) => {
  const role = url.searchParams.get("set");
  if (role === "marketing" || role === "support") {
    cookies.set("discount_role", role, { path: "/", sameSite: "lax", maxAge: 60 * 60 * 24 * 365, httpOnly: false });
  }
  const back = url.searchParams.get("back") ?? "/";
  return redirect(back.startsWith("/") && !back.startsWith("//") ? back : "/", 303);
};
