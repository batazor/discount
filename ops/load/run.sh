#!/usr/bin/env bash
# Load profile of the local stack (spec `load-profile`): seed the catalogue, run
# ops/load/discount.js from the grafana/k6 image on the compose network, leave the
# summary in ops/load/out/. Thresholds fail the run (non-zero exit).
#
#   ops/load/run.sh                 # RATE=50 req/s for DURATION=2m
#   RATE=200 DURATION=5m ops/load/run.sh
set -euo pipefail

SERVICE=${SERVICE:-http://localhost:8080}       # as reached from this shell (seeding)
BASE_URL=${BASE_URL:-http://discount:8080}      # as reached from the k6 container
NETWORK=${NETWORK:-discount_default}
RATE=${RATE:-50}
DURATION=${DURATION:-2m}
DIR=$(cd "$(dirname "$0")" && pwd)

fail() { echo "FAIL: $*" >&2; exit 1; }
curl -sf "$SERVICE/healthz" >/dev/null || fail "service not healthy at $SERVICE (docker compose -f ops/compose.yaml up -d)"

# Same catalogue as the smoke, with a budget on P1 large enough not to clamp during a run.
put() { curl -s -o /dev/null -w "%{http_code}" -H 'Content-Type: application/json' -X PUT "$SERVICE$1" -d "$2"; }
[ "$(put /v1/groups/g '{"group_id":"g","priority":0,"mode":{"type":"Stackable"}}')" = "200" ] || fail "put group"
[ "$(put /v1/promotions/P1 '{"promotion_id":"P1","group_id":"g","priority":10,"exclusive":false,"cap":null,"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[],"action":{"type":"OrderPercentage","bps":1000}}')" = "200" ] || fail "put P1"
[ "$(put /v1/promotions/P2 '{"promotion_id":"P2","group_id":"g","priority":5,"exclusive":false,"cap":{"amount":300,"currency":"EUR"},"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[],"action":{"type":"OrderFixedAmount","amount":{"amount":500,"currency":"EUR"}}}')" = "200" ] || fail "put P2"
[ "$(put /v1/promotions/P1/budget '{"remaining":{"amount":1000000000,"currency":"EUR"}}')" = "200" ] || fail "put budget"
echo "ok:   catalogue seeded (g, P1 10% with 10M budget, P2 fixed 500 capped at 300)"

mkdir -p "$DIR/out"
echo "k6:   RATE=$RATE req/s, DURATION=$DURATION, target $BASE_URL"
docker run --rm -i --network "$NETWORK" \
  -e BASE_URL="$BASE_URL" -e RATE="$RATE" -e DURATION="$DURATION" \
  -v "$DIR:/scripts:ro" -v "$DIR/out:/out" \
  grafana/k6:latest run --quiet /scripts/discount.js
echo "ok:   summary in ops/load/out/summary.json"
