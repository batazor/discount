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

# Seed the catalogue (idempotent PUTs): one group, a 10 % promotion with a budget, a
# capped fixed-amount promotion. With Postgres the catalogue starts empty.
put() { curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -X PUT "$SERVICE$1" -d "$2"; }
[ "$(put /v1/groups/g '{"group_id":"g","priority":0,"mode":{"type":"Stackable"}}')" = "200" ] || fail "put group"
[ "$(put /v1/promotions/P1 '{"promotion_id":"P1","group_id":"g","priority":10,"exclusive":false,"cap":null,"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[],"action":{"type":"OrderPercentage","bps":1000}}')" = "200" ] || fail "put P1"
[ "$(put /v1/promotions/P2 '{"promotion_id":"P2","group_id":"g","priority":5,"exclusive":false,"cap":{"amount":300,"currency":"EUR"},"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[],"action":{"type":"OrderFixedAmount","amount":{"amount":500,"currency":"EUR"}}}')" = "200" ] || fail "put P2"
B=$(curl -s "$SERVICE/v1/promotions/P1/budget")
if [ "$B" = '{"remaining":null}' ]; then
  [ "$(put /v1/promotions/P1/budget '{"remaining":{"amount":1000000,"currency":"EUR"}}')" = "200" ] || fail "put budget"
fi
pass "catalogue seeded (g, P1 10% with budget, P2 fixed 500 capped at 300)"

# Fixture carts: the sample cart (priced to 749) and an invalid one (lowercase currency).
post() { curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$1"; }
CUSTOMER='{"customer_id":"smoke","completed_orders":0,"uses":[]}'
CART='{"cart":{"currency":"EUR","lines":[{"line_id":"L1","unit_price":{"amount":1999,"currency":"EUR"},"quantity":2},{"line_id":"L2","unit_price":{"amount":501,"currency":"EUR"},"quantity":1}],"tags":["#ios"],"attributes":[],"coupon_codes":[]},"customer":'"$CUSTOMER"'}'
BAD='{"cart":{"currency":"eur","lines":[],"tags":[],"attributes":[],"coupon_codes":[]},"customer":'"$CUSTOMER"'}'
for _ in 1 2 3 4 5; do
  [ "$(post "$CART")" = "200" ] || fail "evaluate did not return 200"
done
[ "$(post "$BAD")" = "422" ] || fail "invalid cart did not return 422"
pass "posted 5 valid and 1 invalid evaluation"

# Redemption round trip: reserve, idempotent repeat, confirm, cancel.
ORDER="smoke-$(date +%s)"
REDEEM="{\"order_id\":\"$ORDER\",$(echo "$CART" | cut -c2-)"
R1=$(curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/redeem" -d "$REDEEM")
echo "$R1" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d["status"]=="reserved", d; assert d["result"]["total_discount"]["amount"]>=0' || fail "redeem did not reserve: $R1"
R2=$(curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/redeem" -d "$REDEEM")
[ "$R1" = "$R2" ] || fail "repeated redeem is not idempotent"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVICE/v1/redemptions/$ORDER/confirm")" = "200" ] || fail "confirm failed"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVICE/v1/redemptions/$ORDER/confirm")" = "409" ] || fail "second confirm should be 409"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X POST "$SERVICE/v1/redemptions/$ORDER/cancel")" = "200" ] || fail "cancel failed"
pass "redeem -> idempotent repeat -> confirm -> 409 -> cancel for order $ORDER"

# Coupon lifecycle: a coupon-only promotion, one generated single-use code, redeemed twice.
[ "$(put /v1/promotions/P3 '{"promotion_id":"P3","group_id":"g","priority":1,"exclusive":false,"cap":null,"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[{"type":"RequiresCoupon"}],"action":{"type":"OrderFixedAmount","amount":{"amount":100,"currency":"EUR"}}}')" = "200" ] || fail "put P3"
GEN=$(curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/coupons/generate" -d '{"promotion_id":"P3","count":1,"prefix":"SMOKE-","starts_at":null,"ends_at":null,"max_uses":1,"customer_id":null}')
CODE=$(echo "$GEN" | python3 -c 'import sys,json; d=json.load(sys.stdin); c=d["codes"]; assert len(c)==1 and c[0].startswith("SMOKE-"), d; print(c[0])') || fail "generate failed: $GEN"
CCART=$(echo "$CART" | sed "s/\"coupon_codes\":\[\]/\"coupon_codes\":[\"$CODE\"]/")
C1=$(curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/redeem" -d "{\"order_id\":\"$ORDER-c1\",$(echo "$CCART" | cut -c2-)")
echo "$C1" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert any(a["promotion_id"]=="P3" for a in d["result"]["applied"]), d' || fail "first use of $CODE did not apply P3: $C1"
C2=$(curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/redeem" -d "{\"order_id\":\"$ORDER-c2\",$(echo "$CCART" | cut -c2-)")
echo "$C2" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert any(s["promotion_id"]=="P3" and s["reason"]["type"]=="NotEligible" for s in d["result"]["skipped"]), d' || fail "second use of $CODE was not skipped: $C2"
U=$(curl -s "$SERVICE/v1/coupons/$CODE" | python3 -c 'import sys,json; print(json.load(sys.stdin)["uses"])')
[ "$U" = "1" ] || fail "coupon uses should be 1, got $U"
pass "single-use code $CODE: applied once, skipped on reuse, uses = 1"

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
