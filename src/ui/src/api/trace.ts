// Where an operator can open a trace (spec admin-ui, "API errors are shown verbatim"): the
// Grafana of the observability stack, reachable from the operator's browser.
export const GRAFANA_URL = (process.env.DISCOUNT_GRAFANA_URL ?? "http://localhost:3000").replace(/\/$/, "");

/** Grafana Explore opened on the Tempo data source with `traceId` as the query. */
export function traceLink(traceId: string): string {
  const panes = {
    t: {
      datasource: "tempo",
      queries: [{ refId: "A", datasource: { type: "tempo", uid: "tempo" }, queryType: "traceql", query: traceId }],
    },
  };
  return `${GRAFANA_URL}/explore?schemaVersion=1&orgId=1&panes=${encodeURIComponent(JSON.stringify(panes))}`;
}
