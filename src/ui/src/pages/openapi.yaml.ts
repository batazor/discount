import type { APIRoute } from "astro";
import { API_URL } from "../api/client";

// The service's contract, passed through so the docs page works from the browser even
// when the service's own address is internal to the stack.
export const GET: APIRoute = async () => {
  try {
    const r = await fetch(`${API_URL}/openapi.yaml`);
    return new Response(await r.text(), { status: r.status, headers: { "content-type": r.headers.get("content-type") ?? "application/yaml" } });
  } catch {
    return new Response(`# service unreachable at ${API_URL}\n`, { status: 502, headers: { "content-type": "text/plain" } });
  }
};
