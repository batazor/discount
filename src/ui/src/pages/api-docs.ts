import type { APIRoute } from "astro";
import { API_URL } from "../api/client";

// Swagger UI served by the service, passed through under the console's own origin; its
// document comes from /openapi.yaml, which is passed through as well.
export const GET: APIRoute = async () => {
  try {
    const r = await fetch(`${API_URL}/docs`);
    const html = (await r.text()).replaceAll(`${API_URL}/openapi.yaml`, "/openapi.yaml");
    return new Response(html, { status: r.status, headers: { "content-type": r.headers.get("content-type") ?? "text/html; charset=utf-8" } });
  } catch {
    return new Response(`<!doctype html><p>The discount service at ${API_URL} is unreachable, so its API docs cannot be shown.</p>`, { status: 502, headers: { "content-type": "text/html; charset=utf-8" } });
  }
};
