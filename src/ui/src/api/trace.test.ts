import { describe, expect, it } from "vitest";
import { GRAFANA_URL, traceLink } from "./trace";

describe("traceLink", () => {
  it("opens Grafana Explore on Tempo with the trace id as the query", () => {
    const id = "4bf92f3577b34da6a3ce929d0e0e4736";
    const url = new URL(traceLink(id));
    expect(url.origin + url.pathname).toBe(`${GRAFANA_URL}/explore`);
    const panes = JSON.parse(url.searchParams.get("panes")!);
    const q = panes.t.queries[0];
    expect(panes.t.datasource).toBe("tempo");
    expect(q.queryType).toBe("traceql");
    expect(q.query).toBe(id);
  });
});
