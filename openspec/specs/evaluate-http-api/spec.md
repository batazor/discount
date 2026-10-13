# evaluate-http-api Specification

## Purpose
The HTTP edge of the discount service: how a caller asks for a cart to be priced and what
it gets back, including validation errors. Nothing here changes pricing semantics.

## Requirements

### Requirement: Evaluate endpoint
The service SHALL expose `POST /v1/evaluate` accepting a JSON body with `cart`
(a `CartSnapshot`) and `customer` (a `CustomerContext`), both encoded exactly as in
`docs/ubiquitous-language.md`. The service SHALL load the promotions and groups from its
promotion repository, take `now` from its clock, apply the configured `PricingPolicy`, and
respond `200 OK` with the `PricingResult` JSON on success.

#### Scenario: Successful evaluation
- **WHEN** a valid cart is posted
- **THEN** the response is `200` with a `PricingResult` whose `final_total` equals
  `original_subtotal - total_discount`, and the body is identical to what
  `discount_core::apply` returns for the same input

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
The service SHALL read its listen address, the OTLP endpoint, the promotion fixture path
and the `PricingPolicy` defaults from environment variables with documented names and
defaults (`DISCOUNT_LISTEN=0.0.0.0:8080`, `OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317`,
`DISCOUNT_PROMOTIONS=deploy/promotions.json`, `DISCOUNT_ROUNDING=Down`,
`DISCOUNT_STACKING=Sequential`).

#### Scenario: Defaults
- **WHEN** the service starts with no environment variables set
- **THEN** it listens on `0.0.0.0:8080` and uses `Down` rounding and `Sequential` stacking
