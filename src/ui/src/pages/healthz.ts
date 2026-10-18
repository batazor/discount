import type { APIRoute } from "astro";

// The console's own liveness (design.md, decision 8): never calls the service, so a
// compose health check tells "console down" from "service down".
export const GET: APIRoute = () =>
  new Response(JSON.stringify({ status: "ok" }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
