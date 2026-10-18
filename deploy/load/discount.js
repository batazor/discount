// k6 load profile of the discount service (spec `load-profile`). Four open-model scenarios
// share RATE requests per second: evaluate 70 %, redeem lifecycle 20 %, idempotent repeat
// 5 %, invalid cart 5 %. Thresholds mirror deploy/grafana/alerting/discount.yaml.
//
// Run through deploy/load/run.sh (seeds the catalogue, runs this from the grafana/k6 image).
import http from "k6/http";
import { check } from "k6";
import { textSummary } from "https://jslib.k6.io/k6-summary/0.1.0/index.js";

const BASE = __ENV.BASE_URL || "http://discount:8080";
const RATE = Number(__ENV.RATE || 50);
const DURATION = __ENV.DURATION || "2m";
const PRE = Math.max(RATE, 20);

function share(pct) {
  return Math.max(1, Math.round((RATE * pct) / 100));
}

export const options = {
  discardResponseBodies: false,
  scenarios: {
    evaluate: { executor: "constant-arrival-rate", exec: "evaluate", rate: share(70), timeUnit: "1s", duration: DURATION, preAllocatedVUs: PRE, maxVUs: PRE * 4, tags: { name: "evaluate" } },
    redeem: { executor: "constant-arrival-rate", exec: "redeem", rate: share(20), timeUnit: "1s", duration: DURATION, preAllocatedVUs: PRE, maxVUs: PRE * 4, tags: { name: "redeem" } },
    repeat: { executor: "constant-arrival-rate", exec: "repeat", rate: share(5), timeUnit: "1s", duration: DURATION, preAllocatedVUs: 10, maxVUs: 40, tags: { name: "repeat" } },
    invalid: { executor: "constant-arrival-rate", exec: "invalid", rate: share(5), timeUnit: "1s", duration: DURATION, preAllocatedVUs: 10, maxVUs: 40, tags: { name: "invalid" } },
  },
  thresholds: {
    // Alert "HTTP 5xx ratio per route" is 5 %; http_req_failed counts 5xx only (see below).
    http_req_failed: ["rate<0.05"],
    // Alert "Redeem p95 latency" is 500 ms.
    "http_req_duration{route:redeem}": ["p(95)<500"],
    // First stated target for pure pricing.
    "http_req_duration{route:evaluate}": ["p(95)<200"],
    "http_req_duration{route:confirm}": ["p(95)<500"],
    checks: ["rate>0.99"],
  },
};

// Only 5xx and transport errors count as failures; 422 and 409 are expected answers.
http.setResponseCallback(http.expectedStatuses({ min: 200, max: 499 }));

const HEADERS = { "Content-Type": "application/json" };
const CUSTOMER = { customer_id: "load", completed_orders: 3, uses: [], attributes: [], allowlisted: [] };

function cart(seed) {
  // 1..4 lines, prices 5.01..49.99, quantities 1..3; subtotal varies so caps and
  // percentages both get exercised.
  const lines = [];
  const n = 1 + (seed % 4);
  for (let i = 0; i < n; i++) {
    lines.push({ line_id: `L${i + 1}`, unit_price: { amount: 501 + ((seed * 7919 + i * 104729) % 4499), currency: "EUR" }, quantity: 1 + ((seed + i) % 3) });
  }
  return { currency: "EUR", lines, tags: seed % 2 ? ["#ios"] : [], attributes: [], coupon_codes: [] };
}

function post(path, body, route) {
  return http.post(`${BASE}${path}`, JSON.stringify(body), { headers: HEADERS, tags: { route } });
}

export function evaluate() {
  const r = post("/v1/evaluate", { cart: cart(__ITER + __VU * 13), customer: CUSTOMER }, "evaluate");
  check(r, {
    "evaluate 200": (x) => x.status === 200,
    "evaluate has total_discount": (x) => x.status === 200 && x.json("total_discount.amount") !== undefined,
  });
}

export function redeem() {
  const order = `load-${__VU}-${__ITER}-${Date.now()}`;
  const r = post("/v1/redeem", { order_id: order, cart: cart(__ITER + __VU * 17), customer: CUSTOMER }, "redeem");
  const ok = check(r, { "redeem 200 reserved": (x) => x.status === 200 && x.json("status") === "reserved" });
  if (!ok) return;
  const next = __ITER % 5 === 0 ? "cancel" : "confirm";
  const t = http.post(`${BASE}/v1/redemptions/${order}/${next}`, null, { tags: { route: next } });
  check(t, { [`${next} 200`]: (x) => x.status === 200 });
}

export function repeat() {
  // The same order twice: the second reserve must be idempotent and answer the same body.
  const order = `repeat-${__VU}-${__ITER}-${Date.now()}`;
  const body = { order_id: order, cart: cart(__ITER + __VU * 19), customer: CUSTOMER };
  const a = post("/v1/redeem", body, "redeem");
  const b = post("/v1/redeem", body, "repeat");
  check(b, {
    "repeat 200": (x) => x.status === 200,
    "repeat same body": (x) => a.status === 200 && x.body === a.body,
  });
}

export function invalid() {
  const r = post("/v1/evaluate", { cart: { currency: "eur", lines: [], tags: [], attributes: [], coupon_codes: [] }, customer: CUSTOMER }, "invalid");
  check(r, { "invalid 422": (x) => x.status === 422 });
}

export function handleSummary(data) {
  return {
    stdout: textSummary(data, { indent: " ", enableColors: false }),
    "/out/summary.json": JSON.stringify(data, null, 2),
  };
}
