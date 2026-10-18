# evaluate-http-api Specification

## Purpose
The HTTP edge of the discount service: how a caller asks for a cart to be priced and what
it gets back, including validation errors. Nothing here changes pricing semantics.

## Requirements

### Requirement: Evaluate endpoint
The service SHALL expose `POST /v1/evaluate` accepting a JSON body with `cart`
(a `CartSnapshot`, including `tags`, `attributes` and `coupon_codes`) and `customer`
(a `CustomerContext` with `customer_id`, `completed_orders` and `uses`), both encoded exactly
as in `docs/ubiquitous-language.md`. The service SHALL load the policy, groups,
promotions, the coupons matching `cart.coupon_codes` and the exchange rates from its
catalogue, take `now` from its clock, fill `customer.uses` from reserved and confirmed
redemptions of `customer.customer_id` (adding to any counts the caller supplied), and
respond `200 OK` with the `PricingResult` JSON on success.

#### Scenario: Successful evaluation
- **WHEN** a valid cart is posted
- **THEN** the response is `200` with a `PricingResult` whose `final_total` equals
  `original_subtotal - total_discount`, and the body is identical to what
  `discount_domain::apply` returns for the same assembled input

#### Scenario: Uses come from redemptions
- **WHEN** customer `c1` has two reserved redemptions that applied `P1`, `P1` carries
  `MaxUsesPerCustomer { max: 2 }`, and `c1` evaluates a cart with `uses` empty
- **THEN** `P1` is skipped as `NotEligible`

### Requirement: Pricing errors are reported as structured errors
The service SHALL map `PricingError` to `422 Unprocessable Content` with the body
`{"error": <PricingError JSON>}`, using the same encoding as the reference executable.

#### Scenario: Invalid cart currency
- **WHEN** a cart with currency `"eur"` is posted
- **THEN** the response is `422` with body `{"error": {"type": "InvalidCurrency", "field": "cart.currency"}}`

### Requirement: Malformed requests
The service SHALL respond `400 Bad Request` with `{"decode_error": "<message>"}` when the
body is not valid JSON or does not match the request schema (unknown keys included).

#### Scenario: Unknown key
- **WHEN** the body contains a key not in the schema
- **THEN** the response is `400` and names the offending key in `decode_error`

### Requirement: Health endpoint
The service SHALL expose `GET /healthz` responding `200` with `{"status": "ok"}` while it is
able to serve evaluations.

#### Scenario: Liveness
- **WHEN** `GET /healthz` is requested
- **THEN** the response is `200` with `{"status": "ok"}`

### Requirement: Configuration
The service SHALL read its listen address, the OTLP endpoint, the database URL, the
promotion fixture path and the `PricingPolicy` defaults from environment variables with
documented names and defaults (`DISCOUNT_LISTEN=0.0.0.0:8080`,
`OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317`, `DATABASE_URL` unset,
`DISCOUNT_PROMOTIONS=ops/promotions.json`, `DISCOUNT_ROUNDING=Down`,
`DISCOUNT_STACKING=Sequential`). With `DATABASE_URL` set the catalogue is Postgres and the
policy comes from the database (seeded from the defaults on first start); without it the
catalogue is in memory, seeded from the fixture.

#### Scenario: Defaults
- **WHEN** the service starts with no environment variables set
- **THEN** it listens on `0.0.0.0:8080`, uses the in-memory catalogue from the fixture,
  `Down` rounding and `Sequential` stacking

#### Scenario: Database configured
- **WHEN** `DATABASE_URL` is set
- **THEN** promotions come from Postgres and the fixture file is not read

### Requirement: The OpenAPI document is the contract
`api/openapi.yaml` SHALL describe every route the service serves, with the ubiquitous
language JSON shapes and the documented status codes. The service SHALL serve the same
document at `GET /openapi.yaml` and Swagger UI at `GET /docs`. A client generated from the
document SHALL be able to drive evaluation, redemption lifecycle and catalogue management
against a running service.

#### Scenario: Every route is documented
- **WHEN** the router is compared with the document
- **THEN** every path the router serves appears in the document

#### Scenario: Generated client round trip
- **WHEN** the generated client puts a group and a promotion, evaluates a cart, redeems an
  order, confirms it and reads the redemption back
- **THEN** every call succeeds with the documented status and the decoded bodies match the
  server's
