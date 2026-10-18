#!/usr/bin/env bash
# Smoke check of the local stack: post the fixture carts to discount-server, check the
# operator console answers, then verify through Grafana's data source proxies that the
# metric, a trace and a log arrived. Exit non-zero on any failure.
set -euo pipefail

SERVICE=${SERVICE:-http://localhost:8080}
GRAFANA=${GRAFANA:-http://localhost:3000}
UI=${UI:-http://localhost:4321}
DIR=$(cd "$(dirname "$0")" && pwd)

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok:   $*"; }

curl -sf "$SERVICE/healthz" >/dev/null || fail "service not healthy at $SERVICE"
STARTED=$(date +%s)

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

# Operator console: alive on its own, and its promotions page shows the seeded catalogue.
[ "$(curl -s -o /dev/null -w '%{http_code}' "$UI/healthz")" = "200" ] || fail "console not healthy at $UI"
curl -s "$UI/promotions" | grep -q 'href="/promotions/P1"' || fail "console promotions page does not list P1"
pass "console at $UI is up and lists P1"

# Fixture carts: the sample cart (priced to 749) and an invalid one (lowercase currency).
post() { curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$1"; }
CUSTOMER='{"customer_id":"smoke","completed_orders":0,"uses":[],"attributes":[],"allowlisted":[]}'
CART='{"cart":{"currency":"EUR","lines":[{"line_id":"L1","unit_price":{"amount":1999,"currency":"EUR"},"quantity":2},{"line_id":"L2","unit_price":{"amount":501,"currency":"EUR"},"quantity":1}],"tags":["#ios"],"attributes":[],"coupon_codes":[]},"customer":'"$CUSTOMER"'}'
BAD='{"cart":{"currency":"eur","lines":[],"tags":[],"attributes":[],"coupon_codes":[]},"customer":'"$CUSTOMER"'}'
for _ in 1 2 3 4 5; do
  [ "$(post "$CART")" = "200" ] || fail "evaluate did not return 200"
done
[ "$(post "$BAD")" = "422" ] || fail "invalid cart did not return 422"
pass "posted 5 valid and 1 invalid evaluation"

# One evaluation inside a caller's trace (W3C traceparent); the trace id must show up in
# Tempo under that id, with the service's spans as children.
TRACE_ID=$(python3 -c 'import secrets; print(secrets.token_hex(16))')
[ "$(curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -H "traceparent: 00-$TRACE_ID-00f067aa0ba902b7-01" -X POST "$SERVICE/v1/evaluate" -d "$CART")" = "200" ] || fail "evaluate with traceparent did not return 200"
pass "posted one evaluation with traceparent $TRACE_ID"

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

# Customer coupons: the code the smoke customer just used is listed as applied with its order.
L=$(curl -s "$SERVICE/v1/customers/smoke/coupons")
echo "$L" | python3 -c '
import sys,json
code, order = sys.argv[1], sys.argv[2]
d = json.load(sys.stdin)
e = [x for x in d if x["coupon"]["code"] == code]
assert e and e[0]["status"] == "applied" and e[0]["order_id"] == order, d
' "$CODE" "$ORDER-c1" || fail "customer listing did not show $CODE as applied: $L"
[ "$(curl -s -o /dev/null -w '%{http_code}' -X PUT "$SERVICE/v1/customers/smoke/coupons/NOPE")" = "404" ] || fail "saving an unknown code should be 404"
pass "customer smoke sees $CODE as applied in order $ORDER-c1"

# Promotion metadata: title and source on P3, visible in the customer's listing and in the filters.
[ "$(put /v1/promotions/P3/metadata '{"title":"Smoke code","description":"single use","source":"hotels","labels":{"channel":"smoke"}}')" = "200" ] || fail "put metadata"
curl -s "$SERVICE/v1/customers/smoke/coupons?source=hotels" | python3 -c '
import sys,json
code = sys.argv[1]
d = json.load(sys.stdin)
e = [x for x in d if x["coupon"]["code"] == code]
assert e and e[0]["promotion"]["metadata"]["title"] == "Smoke code", d
' "$CODE" || fail "customer listing lacks the metadata of $CODE"
N=$(curl -s "$SERVICE/v1/customers/smoke/coupons?source=transfers" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))')
[ "$N" = "0" ] || fail "source filter should exclude other sources, got $N entries"
[ "$(curl -s "$SERVICE/v1/promotions?source=hotels" | python3 -c 'import sys,json; print(",".join(p["promotion_id"] for p in json.load(sys.stdin)))')" = "P3" ] || fail "promotion source filter"
pass "metadata on P3 reaches the customer listing; source filters work"

# Allowlist: P4 applies only to listed customers.
[ "$(put /v1/promotions/P4 '{"promotion_id":"P4","group_id":"g","priority":2,"exclusive":false,"cap":null,"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[{"type":"RequiresAllowlist"}],"action":{"type":"OrderFixedAmount","amount":{"amount":100,"currency":"EUR"}}}')" = "200" ] || fail "put P4"
[ "$(put /v1/promotions/P4/allowlist '{"customer_ids":["smoke"]}')" = "200" ] || fail "put allowlist"
curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$CART" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert any(a["promotion_id"]=="P4" for a in d["applied"]), d' || fail "P4 did not apply to the listed customer"
STRANGER=$(echo "$CART" | sed 's/"customer_id":"smoke"/"customer_id":"stranger"/')
curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$STRANGER" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert any(s["promotion_id"]=="P4" and s["reason"]["type"]=="NotEligible" for s in d["skipped"]), d' || fail "P4 applied to an unlisted customer"
pass "allowlist on P4: applies to smoke, skipped for stranger"

# States: pausing P1 takes it out of the evaluation and brings it back on resume.
[ "$(put /v1/promotions/P1/state '{"state":"paused"}')" = "200" ] || fail "pause P1"
curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$CART" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert all(a["promotion_id"]!="P1" for a in d["applied"]) and all(s["promotion_id"]!="P1" for s in d["skipped"]), d' || fail "paused P1 still appears in the evaluation"
[ "$(curl -s "$SERVICE/v1/promotions?state=paused" | python3 -c 'import sys,json; print(",".join(p["promotion_id"] for p in json.load(sys.stdin)))')" = "P1" ] || fail "state filter"
[ "$(put /v1/promotions/P1/state '{"state":"active"}')" = "200" ] || fail "resume P1"
curl -s -H 'Content-Type: application/json' -X POST "$SERVICE/v1/evaluate" -d "$CART" | python3 -c 'import sys,json; d=json.load(sys.stdin); assert any(a["promotion_id"]=="P1" for a in d["applied"]), d' || fail "resumed P1 does not apply"
pass "P1 paused and resumed: absent from the evaluation while paused"

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

tempo_by_name() { # tempo_by_name <span name>: traces with a span of that exact name
  local now start q
  now=$(date +%s); start=$((now-900))
  q=$(python3 -c 'import sys,urllib.parse; print(urllib.parse.quote("{name=\"%s\"}" % sys.argv[1]))' "$1")
  curl -s "$GRAFANA/api/datasources/proxy/uid/tempo/api/search?q=$q&start=$start&end=$now&limit=5" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d.get("traces",[])))'
}

tempo_propagated() { # the trace we started with traceparent, containing the evaluate span
  curl -s "$GRAFANA/api/datasources/proxy/uid/tempo/api/traces/$TRACE_ID" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(sum(1 for b in d.get("batches",[]) for ss in b["scopeSpans"] for s in ss["spans"] if s["name"]=="discount.evaluate"))'
}

tempo_healthz() { # must stay 0 since this run started: health checks are not traced
  local now q
  now=$(date +%s)
  q=$(python3 -c 'import urllib.parse; print(urllib.parse.quote("{name=\"GET /healthz\"}"))')
  # Tempo's start/end are block-granular for recent data; filter on the trace start too.
  curl -s "$GRAFANA/api/datasources/proxy/uid/tempo/api/search?q=$q&start=$STARTED&end=$now&limit=100" \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); s=int(sys.argv[1])*10**9; print(sum(1 for t in d.get("traces",[]) if int(t["startTimeUnixNano"])>=s))' "$STARTED"
}

retry "Tempo traces for service.name=discount" tempo_count
retry "Tempo span named POST /v1/redemptions/{order_id}/confirm" tempo_by_name "POST /v1/redemptions/{order_id}/confirm"
retry "Tempo trace $TRACE_ID carries discount.evaluate" tempo_propagated
H=$(tempo_healthz 2>/dev/null || echo 0)
[ "${H:-0}" = "0" ] || fail "GET /healthz traces found since this run started: $H"
pass "no GET /healthz traces"
retry "Loki evaluation log lines for service_name=discount" loki_count

spanmetrics_count() { # Tempo's metrics generator has seen the evaluate server span
  curl -s "$GRAFANA/api/datasources/proxy/uid/prometheus/api/v1/query?query=sum(traces_spanmetrics_calls_total%7Bservice%3D%22discount%22%2Cspan_name%3D%22POST%20%2Fv1%2Fevaluate%22%7D)" \
    | python3 -c 'import sys,json; r=json.load(sys.stdin)["data"]["result"]; print(len(r))'
}
retry "span metrics for POST /v1/evaluate" spanmetrics_count

# Alert rules as code: the five discount-* rules are provisioned into folder Discount.
R=$(curl -s "$GRAFANA/api/v1/provisioning/alert-rules" | python3 -c 'import sys,json; print(sum(1 for r in json.load(sys.stdin) if r["uid"].startswith("discount-")))' 2>/dev/null || echo 0)
[ "${R:-0}" -ge 5 ] || fail "expected at least 5 provisioned discount-* alert rules, found $R"
pass "provisioned alert rules: $R"
SINK=${SINK:-http://localhost:9095}
curl -sf "$SINK/" | python3 -c 'import sys,json; assert isinstance(json.load(sys.stdin), list)' 2>/dev/null || fail "alert sink at $SINK does not answer GET / with a JSON list"
curl -s "$GRAFANA/api/v1/provisioning/contact-points" | python3 -c 'import sys,json; assert any(c["name"]=="discount-sink" for c in json.load(sys.stdin))' 2>/dev/null || fail "contact point discount-sink is not provisioned"
pass "alert sink up and contact point discount-sink provisioned"

echo "smoke passed"
