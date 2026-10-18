// Spec `telemetry`, "Console calls carry trace context": every call sends a W3C
// traceparent and reports its trace id on the outcome.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, call, newTraceparent } from "./client";

const TRACEPARENT = /^00-([0-9a-f]{32})-([0-9a-f]{16})-01$/;

type Seen = { url: string; traceparent: string | null };
const seen: Seen[] = [];

function fakeFetch(status: number, body: unknown, delayMs = 0) {
  return vi.fn(async (input: Request | string | URL) => {
    const req = input instanceof Request ? input : new Request(String(input));
    seen.push({ url: req.url, traceparent: req.headers.get("traceparent") });
    if (delayMs) await new Promise((r) => setTimeout(r, delayMs));
    return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
  });
}

beforeEach(() => {
  seen.length = 0;
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("newTraceparent", () => {
  it("mints a W3C header with random ids", () => {
    const a = newTraceparent();
    const b = newTraceparent();
    expect(a.header).toMatch(TRACEPARENT);
    expect(a.header).toBe(`00-${a.traceId}-${a.parentId}-01`);
    expect(a.traceId).not.toBe(b.traceId);
  });
});

describe("call", () => {
  it("sends traceparent and reports the trace id on an ok outcome", async () => {
    vi.stubGlobal("fetch", fakeFetch(200, { status: "ok" }));
    const o = await call(() => api.GET("/healthz"));
    expect(o.ok).toBe(true);
    const m = seen[0].traceparent?.match(TRACEPARENT);
    expect(m).not.toBeNull();
    expect(o.traceId).toBe(m![1]);
  });

  it("keeps the trace id on a documented error", async () => {
    vi.stubGlobal("fetch", fakeFetch(422, { error: { type: "InvalidCurrency" } }));
    const o = await call(() => api.GET("/healthz"));
    expect(o.ok).toBe(false);
    if (o.ok || o.unreachable) throw new Error("expected an API error");
    expect(o.status).toBe(422);
    expect(o.body).toEqual({ error: { type: "InvalidCurrency" } });
    expect(o.traceId).toBe(seen[0].traceparent!.split("-")[1]);
  });

  it("reports a trace id even when the service is unreachable", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("fetch failed");
      }),
    );
    const o = await call(() => api.GET("/healthz"));
    expect(o.ok).toBe(false);
    if (o.ok || !o.unreachable) throw new Error("expected unreachable");
    expect(o.traceId).toMatch(/^[0-9a-f]{32}$/);
  });

  it("gives interleaved calls distinct trace ids, each on its own request", async () => {
    vi.stubGlobal("fetch", fakeFetch(200, { status: "ok" }, 5));
    const [a, b] = await Promise.all([call(() => api.GET("/healthz")), call(() => api.GET("/healthz"))]);
    expect(a.traceId).not.toBe(b.traceId);
    const sent = seen.map((s) => s.traceparent!.split("-")[1]).sort();
    expect(sent).toEqual([a.traceId, b.traceId].sort());
  });
});
