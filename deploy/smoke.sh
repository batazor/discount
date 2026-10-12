#!/usr/bin/env bash
# Smoke check of the local observability chain: post the fixture carts to discount-server,
# then verify through Grafana's data source proxies that the metric, a trace and a log
# arrived. Exit non-zero on any failure.
set -euo pipefail

SERVICE=${SERVICE:-http://localhost:8080}
GRAFANA=${GRAFANA:-http://localhost:3000}
DIR=$(cd "$(dirname "$0")" && pwd)

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok:   $*"; }

curl -sf "$SERVICE/healthz" >/dev/null || fail "service not healthy at $SERVICE"

# Fixture carts: the sample cart (priced to 749) and an invalid one (lowercase currency).
post() { curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$1"; }
CART='{"cart":{"currency":"EUR","lines":[{"line_id":"L1","unit_price":{"amount":1999,"currency":"EUR"},"quantity":2},{"line_id":"L2","unit_price":{"amount":501,"currency":"EUR"},"quantity":1}]},"customer":{}}'
BAD='{"cart":{"currency":"eur","lines":[]},"customer":{}}'
for _ in 1 2 3 4 5; do
  [ "$(post "$CART")" = "200" ] || fail "evaluate did not return 200"
done
[ "$(post "$BAD")" = "422" ] || fail "invalid cart did not return 422"
pass "posted 5 valid and 1 invalid evaluation"

# Give the periodic exporters time to flush (metrics export every 60s by default is too
# slow for a smoke; the service sets OTEL_METRIC_EXPORT_INTERVAL=5000).
echo "waiting for export..."
for i in $(seq 1 24); do
  sleep 5
  M=$(curl -s "$GRAFANA/api/datasources/proxy/uid/prometheus/api/v1/query?query=sum(discount_evaluations_total)" | python3 -c 'import sys,json; d=json.load(sys.stdin); r=d["data"]["result"]; print(r[0]["value"][1] if r else "")' 2>/dev/null || true)
  if [ -n "$M" ] && [ "$M" != "0" ]; then break; fi
  [ "$i" = 24 ] && fail "discount_evaluations_total not found in Prometheus"
done
pass "Prometheus has discount_evaluations_total = $M"

# Traces and logs become searchable a little after ingestion; retry for up to two minutes.
retry() { # retry <label> <command producing a count>
  local label=$1; shift
  for i in $(seq 1 24); do
    local n
    n=$("$@" 2>/dev/null || echo 0)
    if [ "${n:-0}" -gt 0 ]; then pass "$label: $n"; return 0; fi
    sleep 5
  done
  fail "$label: nothing found after 120s"
}

tempo_count() {
  local now start
  now=$(date +%s); start=$((now-900))
  curl -s "$GRAFANA/api/datasources/proxy/uid/tempo/api/search?tags=service.name%3Ddiscount&start=$start&end=$now&limit=5" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d.get("traces",[])))'
}

loki_count() {
  local now start
  now=$(date +%s); start=$((now-900))
  curl -s -G "$GRAFANA/api/datasources/proxy/uid/loki/loki/api/v1/query_range" \
    --data-urlencode 'query={service_name="discount"} |= "evaluation"' \
    --data-urlencode "start=${start}000000000" --data-urlencode "end=${now}000000000" --data-urlencode "limit=5" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(sum(len(s["values"]) for s in d["data"]["result"]))'
}

retry "Tempo traces for service.name=discount" tempo_count
retry "Loki evaluation log lines for service_name=discount" loki_count

echo "smoke passed"
