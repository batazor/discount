#!/usr/bin/env python3
"""Make a running discount service look like a business in flight (spec `demo-catalogue`).

Loads deploy/demo/catalogue.json through the HTTP API (groups, promotions, budgets, titles,
allowlists, codes, rates, grant rules), then plays a week of activity: customers with
different tiers and channels place orders, some are confirmed, some stay reserved, some
are cancelled and get an apology code, one leaves a review, a flash campaign runs out.
Every date window is shifted by `now - as_of`, so the catalogue reads "as of today"
whenever it is loaded. Safe to run again: every write is an upsert, every order id is
fixed, and nothing is generated twice.

    docker compose -f deploy/compose.yaml up -d --build
    deploy/demo/seed.py                      # SERVICE=http://localhost:8080

Afterwards open the console (http://localhost:4321) and look up what the script prints.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

SERVICE = os.environ.get("SERVICE", "http://localhost:8080").rstrip("/")
CATALOGUE = Path(os.environ.get("CATALOGUE", Path(__file__).with_name("catalogue.json")))
DAY = 86400
NOW = int(time.time()) // DAY * DAY  # midnight UTC today: stable within a day, so reruns repeat


def call(method, path, body=None, ok=(200,)):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(f"{SERVICE}{path}", data=data, method=method)
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            text = r.read().decode()
            return r.status, (json.loads(text) if text else None)
    except urllib.error.HTTPError as e:
        text = e.read().decode()
        status, payload = e.code, (json.loads(text) if text else None)
        if status not in ok:
            sys.exit(f"FAIL: {method} {path} -> {status} {text[:300]}")
        return status, payload
    except urllib.error.URLError as e:
        sys.exit(f"FAIL: {method} {path}: {e.reason} (is the stack up? docker compose -f deploy/compose.yaml up -d)")


def put(path, body, ok=(200,)):
    status, _ = call("PUT", path, body, ok=ok)
    return status


def shifted(ts, delta):
    return None if ts is None else ts + delta


def shift_rule(rule, delta):
    if rule["type"] in ("DateWindow", "AttributeDateWindow"):
        rule = dict(rule, starts_at=shifted(rule["starts_at"], delta), ends_at=shifted(rule["ends_at"], delta))
    return rule


def money(amount, currency="EUR"):
    return {"amount": amount, "currency": currency}


def line(line_id, unit, qty=1, currency="EUR"):
    return {"line_id": line_id, "unit_price": money(unit, currency), "quantity": qty}


def cart(lines, tags=(), attributes=None, codes=(), currency="EUR"):
    attrs = [{"key": k, "value": str(v)} for k, v in (attributes or {}).items()]
    return {"currency": currency, "lines": lines, "tags": list(tags), "attributes": attrs, "coupon_codes": list(codes)}


def customer(cid, completed=0, **attributes):
    return {
        "customer_id": cid,
        "completed_orders": completed,
        "uses": [],
        "attributes": [{"key": k, "value": str(v)} for k, v in attributes.items()],
        "allowlisted": [],
    }


# ---------------------------------------------------------------- catalogue

def set_rate_source():
    """With RATE_SOURCE_URL set, point the stack at a live provider (the demo needs no
    internet otherwise), e.g. RATE_SOURCE_URL=https://api.frankfurter.app/latest?from=EUR."""
    url = os.environ.get("RATE_SOURCE_URL")
    if not url:
        return
    put("/v1/exchange-rate-source", {"url": url, "interval_seconds": 3600, "max_age_seconds": 86400, "timeout_seconds": 10})
    status, r = call("POST", "/v1/exchange-rate-source/refresh", ok=(200, 502))
    if status == 200:
        print(f"ok:   exchange-rate source {url}: {r['last_count']} pairs fetched")
    else:
        print(f"warn: exchange-rate source {url} set, first fetch failed: {r.get('message')}")


def load_catalogue():
    doc = json.loads(CATALOGUE.read_text())
    delta = NOW - (doc.get("as_of") or NOW)
    print(f"catalogue {CATALOGUE.name}: shifting windows by {delta // DAY} days")

    put("/v1/policy", {"rounding": {"type": "Down"}, "stacking": {"type": "Sequential"}, "order_cap": {"amount": None, "bps": 5000}})
    print("ok:   pricing policy: Down, Sequential, order cap 50 %")

    for g in doc["groups"]:
        put(f"/v1/groups/{g['group_id']}", g)
    print(f"ok:   {len(doc['groups'])} stacking groups")

    budgets = 0
    for p in doc["promotions"]:
        budget = p["remaining_budget"]
        body = dict(p, remaining_budget=None, rules=[shift_rule(r, delta) for r in p["rules"]])
        put(f"/v1/promotions/{p['promotion_id']}", body)
        if budget is not None:
            put(f"/v1/promotions/{p['promotion_id']}/budget", {"remaining": budget})
            budgets += 1
    print(f"ok:   {len(doc['promotions'])} promotions, {budgets} with a budget")

    for m in doc.get("metadata", []):
        put(f"/v1/promotions/{m['promotion_id']}/metadata", m["metadata"])
    for a in doc.get("allowlists", []):
        put(f"/v1/promotions/{a['promotion_id']}/allowlist", {"customer_ids": a["customer_ids"]})
    for c in doc.get("coupons", []):
        body = dict(c, starts_at=shifted(c["starts_at"], delta), ends_at=shifted(c["ends_at"], delta))
        put(f"/v1/coupons/{c['code']}", body)
    for r in doc.get("exchange_rates", []):
        put(f"/v1/exchange-rates/{r['from']}/{r['to']}", {"numerator": r["numerator"], "denominator": r["denominator"]})
    for g in doc.get("grant_rules", []):
        put(f"/v1/grant-rules/{g['rule_id']}", g)
    print(
        f"ok:   {len(doc.get('metadata', []))} titles, {len(doc.get('allowlists', []))} allowlists, "
        f"{len(doc.get('coupons', []))} stored codes, {len(doc.get('exchange_rates', []))} rates, "
        f"{len(doc.get('grant_rules', []))} grant rules"
    )


def generate_codes(promotion_id, prefix, count, max_uses=1):
    """A batch of single-use codes, once: skipped when the prefix already has `count` codes."""
    _, coupons = call("GET", "/v1/coupons")
    have = sum(1 for c in coupons if c["promotion_id"] == promotion_id and c["code"].startswith(prefix))
    if have >= count:
        print(f"ok:   {have} {prefix}* codes already exist for {promotion_id}")
        return
    body = {"promotion_id": promotion_id, "count": count - have, "prefix": prefix, "starts_at": None, "ends_at": None, "max_uses": max_uses, "customer_id": None}
    _, r = call("POST", "/v1/coupons/generate", body)
    print(f"ok:   generated {len(r['codes'])} {prefix}* single-use codes for {promotion_id}")


# ---------------------------------------------------------------- activity

def redeem(order_id, c, cust):
    _, r = call("POST", "/v1/redeem", {"order_id": order_id, "cart": c, "customer": cust})
    applied = ", ".join(f"{a['promotion_id']} -{a['discount']['amount'] / 100:.2f}" for a in r["result"]["applied"]) or "nothing"
    subtotal = r["result"]["original_subtotal"]["amount"] / 100
    print(f"      {order_id:<14} {cust['customer_id']:<7} {subtotal:>9.2f} EUR  {r['status']:<9} {applied}")
    return r


def transition(order_id, step):
    status, _ = call("POST", f"/v1/redemptions/{order_id}/{step}", ok=(200, 409))
    return status


def grant(trigger, customer_id, order_id):
    _, c = call("POST", "/v1/coupons/grant", {"trigger": trigger, "customer_id": customer_id, "order_id": order_id})
    print(f"      grant on {trigger} of {order_id}: {c['code']} for {customer_id}, {c['promotion_id']}")
    return c["code"]


def evaluate(c, cust, expect=200):
    status, r = call("POST", "/v1/evaluate", {"cart": c, "customer": cust}, ok=(200, 422))
    if status != expect:
        sys.exit(f"FAIL: evaluate expected {expect}, got {status}: {r}")
    return r


def play_activity():
    print("orders (reserve, then confirm or cancel; applied campaigns as the service answered):")
    checkin = NOW + 60 * DAY  # inside the resort window

    # ann: gold member, books a resort in the app and a car on the web.
    ann = customer("ann", 14, tier="gold", completed_app_orders=6, country="DE")
    redeem("DEMO-1001", cart([line("room-deluxe", 12900, 3), line("breakfast", 1500, 6)], ["#ios"], {"product": "hotel", "hotel_kind": "resort", "checkin": checkin, "destination": "ES"}), ann)
    transition("DEMO-1001", "confirm")
    redeem("DEMO-1002", cart([line("car-compact", 4500, 4), line("child-seat", 900, 1)], ["#web"], {"product": "car", "pickup": "BCN"}), ann)
    transition("DEMO-1002", "confirm")

    # bob: first order with the public welcome code, still awaiting payment; a transfer he
    # cancels, which earns him an apology code he spends on a city hotel.
    bob = customer("bob", 0, completed_app_orders=0, country="FR")
    put("/v1/customers/bob/coupons/WELCOME15", None)
    redeem("DEMO-1003", cart([line("room-standard", 8900, 2)], ["#android"], {"product": "hotel", "hotel_kind": "city", "destination": "IT"}, ["WELCOME15"]), bob)
    redeem("DEMO-1004", cart([line("transfer-airport", 3500, 1)], ["#android"], {"product": "transfer"}), bob)
    transition("DEMO-1004", "cancel")
    sorry = grant("cancel", "bob", "DEMO-1004")
    redeem("DEMO-1005", cart([line("room-standard", 6900, 2)], ["#android"], {"product": "hotel", "hotel_kind": "city"}, [sorry]), bob)
    transition("DEMO-1005", "confirm")

    # carla: silver member on the web; one car rental confirmed, one cancelled with its code unused.
    carla = customer("carla", 3, tier="silver", completed_app_orders=1, country="PT")
    put("/v1/customers/carla/coupons/WELCOME15", None)
    redeem("DEMO-1006", cart([line("car-suv", 5200, 3)], ["#web"], {"product": "car", "pickup": "LIS"}), carla)
    transition("DEMO-1006", "confirm")
    redeem("DEMO-1007", cart([line("room-standard", 7900, 1)], ["#web"], {"product": "hotel", "hotel_kind": "city"}), carla)
    transition("DEMO-1007", "cancel")
    grant("cancel", "carla", "DEMO-1007")

    # dmitri: three flash orders use the whole FLASH-48H allowance; the fourth is still pending.
    dmitri = customer("dmitri", 7, completed_app_orders=2, country="PL")
    for i, (unit, qty) in enumerate([(9900, 1), (14900, 1), (7900, 2)], start=8):
        redeem(f"DEMO-10{i:02d}", cart([line("room-standard", unit, qty)], ["#flash", "#ios"], {"product": "hotel", "hotel_kind": "city"}), dmitri)
        transition(f"DEMO-10{i:02d}", "confirm")
    redeem("DEMO-1011", cart([line("room-standard", 11900, 1)], ["#flash", "#ios"], {"product": "hotel", "hotel_kind": "city"}), dmitri)

    # eve: partner traffic, priced with the USD partner discount converted at the stored rate.
    eve = customer("eve", 1, country="US")
    redeem("DEMO-1012", cart([line("room-suite", 21900, 2)], ["#partner", "#web"], {"product": "hotel", "hotel_kind": "resort", "checkin": checkin}), eve)
    transition("DEMO-1012", "confirm")

    # frank: a modest stay, then a review that earns a thank-you code he has not used yet.
    frank = customer("frank", 2, completed_app_orders=0, country="NL")
    redeem("DEMO-1013", cart([line("room-standard", 5900, 1)], ["#web"], {"product": "hotel", "hotel_kind": "city"}), frank)
    transition("DEMO-1013", "confirm")
    grant("review", "frank", "DEMO-1013")

    # grace: VIP and gold; a large villa booking, then her personal welcome code.
    grace = customer("grace", 22, tier="platinum", completed_app_orders=9, country="CH")
    redeem("DEMO-1014", cart([line("villa", 25900, 4), line("spa", 4900, 2)], ["#ios"], {"product": "hotel", "hotel_kind": "villa", "checkin": checkin, "destination": "GR"}), grace)
    transition("DEMO-1014", "confirm")
    redeem("DEMO-1015", cart([line("room-standard", 7900, 1)], ["#ios"], {"product": "hotel", "hotel_kind": "city"}, ["WELCOME-GRACE"]), grace)
    transition("DEMO-1015", "confirm")

    # Dry runs: what the dashboards count as evaluations without a redemption, including the
    # rejected carts the validation-errors panel is for.
    anon = customer("walk-in", 0)
    evaluate(cart([line("room-standard", 9900, 1)], ["#web"], {"product": "hotel"}), anon)
    evaluate(cart([line("car-compact", 3900, 2)], ["#android"], {"product": "car"}), customer("henry", 5, tier="silver"))
    evaluate(cart([line("transfer-airport", 2500, 2)], ["#ios"], {"product": "transfer"}), customer("irene", 0, completed_app_orders=0))
    evaluate(cart([line("room-standard", 9900, 1)], currency="eur"), anon, expect=422)
    evaluate(cart([line("room-standard", 9900, 1, currency="USD")]), anon, expect=422)
    evaluate(cart([line("a", 100, 1), line("a", 100, 1)]), anon, expect=422)
    print("ok:   3 dry-run evaluations and 3 rejected carts")


def main():
    status, _ = call("GET", "/healthz", ok=(200,))
    if status != 200:
        sys.exit(f"FAIL: service not healthy at {SERVICE}")
    load_catalogue()
    generate_codes("WELCOME", "NEWS-", 25)
    set_rate_source()
    play_activity()
    print(
        "\nseeded. In the console: Campaigns (live, scheduled, exhausted, ended; FLASH-48H ran out, "
        "CARS-SPENT has no budget left); Find: ann, bob, carla, dmitri, eve, frank, grace, "
        "DEMO-1001 .. DEMO-1015, WELCOME15; Explain from order DEMO-1014."
    )


if __name__ == "__main__":
    main()
