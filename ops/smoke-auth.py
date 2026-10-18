"""Smoke check of the OIDC path (spec `access-control`): tokens from the Keycloak realm
`discount`, then the service with no token, a storefront token and an operator token.
Runs inside the compose network (see compose.auth.yaml). Exit non-zero on any failure."""
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

KEYCLOAK = os.environ.get("KEYCLOAK", "http://keycloak:8080")
SERVICE = os.environ.get("SERVICE", "http://discount:8080")
REALM = f"{KEYCLOAK}/realms/discount"


def fail(msg):
    print(f"FAIL: {msg}")
    sys.exit(1)


def ok(msg):
    print(f"ok:   {msg}")


def call(method, url, body=None, token=None, content_type="application/json"):
    data = body.encode() if isinstance(body, str) else body
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", content_type)
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(req, timeout=10) as res:
            return res.status, res.read().decode()
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()


def token_for(client_id, secret):
    body = urllib.parse.urlencode({"grant_type": "client_credentials", "client_id": client_id, "client_secret": secret})
    status, text = call("POST", f"{REALM}/protocol/openid-connect/token", body, content_type="application/x-www-form-urlencoded")
    if status != 200:
        fail(f"token for {client_id}: {status} {text[:200]}")
    return json.loads(text)["access_token"]


# Wait for the service (it warms up the JWKS from Keycloak before it listens).
for _ in range(60):
    try:
        if call("GET", f"{SERVICE}/healthz")[0] == 200:
            break
    except Exception:
        pass
    time.sleep(2)
else:
    fail("service not healthy")
ok("service and Keycloak are up")

status, text = call("GET", f"{SERVICE}/v1/promotions")
problem = json.loads(text) if text else {}
if status != 401 or problem.get("status") != 401 or problem.get("title") != "Unauthorized":
    fail(f"no token should be a 401 problem, got {status} {text[:120]}")
ok("no token -> 401 problem details")

storefront = token_for("discount-storefront", "storefront-secret")
operator = token_for("discount-operator", "operator-secret")

group = '{"group_id":"g","priority":0,"mode":{"type":"Stackable"}}'
promo = '{"promotion_id":"P1","group_id":"g","priority":10,"exclusive":false,"cap":null,"remaining_budget":null,"uses_total":0,"rounding":null,"rules":[],"action":{"type":"OrderPercentage","bps":1000}}'
status, _ = call("PUT", f"{SERVICE}/v1/groups/g", group, token=storefront)
if status != 403:
    fail(f"storefront must not write the catalogue, got {status}")
ok("storefront token on a catalogue write -> 403")
for path, body in (("/v1/groups/g", group), ("/v1/promotions/P1", promo)):
    status, text = call("PUT", f"{SERVICE}{path}", body, token=operator)
    if status != 200:
        fail(f"operator PUT {path}: {status} {text[:120]}")
ok("operator token seeds the catalogue of tenant hotels")

cart = '{"cart":{"currency":"EUR","lines":[{"line_id":"L1","unit_price":{"amount":1999,"currency":"EUR"},"quantity":2}],"tags":[],"attributes":[],"coupon_codes":[]},"customer":{"customer_id":"smoke","completed_orders":0,"uses":[],"attributes":[],"allowlisted":[]}}'
status, text = call("POST", f"{SERVICE}/v1/evaluate", cart, token=storefront)
if status != 200 or json.loads(text)["total_discount"]["amount"] != 399:
    fail(f"storefront evaluate: {status} {text[:200]}")
ok("storefront token prices in tenant hotels (10 % of 3998 = 399)")
status, _ = call("POST", f"{SERVICE}/v1/evaluate", cart, token=operator)
if status != 403:
    fail(f"operator must not price, got {status}")
ok("operator token on /v1/evaluate -> 403")
status, _ = call("GET", f"{SERVICE}/v1/tenants", token=operator)
if status != 403:
    fail(f"tenants are admin only, got {status}")
ok("tenants are admin only")
print("smoke-auth passed")
