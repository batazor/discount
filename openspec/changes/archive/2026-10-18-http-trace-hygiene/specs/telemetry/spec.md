# Spec Delta

## ADDED Requirements

### Requirement: Server spans are named by route template
Every HTTP server span SHALL be named `<METHOD> <route template>` where the template is the
matched router pattern with its placeholders (for example `POST /v1/redemptions/{order_id}/confirm`,
`PUT /v1/promotions/{promotion_id}`), never the concrete request path. The template SHALL also be
recorded as the span attribute `http.route`. Requests matching no route SHALL be named
`<METHOD>` only.

#### Scenario: Confirming a redemption
- **WHEN** `POST /v1/redemptions/order-42/confirm` is served
- **THEN** the server span is named `POST /v1/redemptions/{order_id}/confirm` and `http.route` is
  `/v1/redemptions/{order_id}/confirm`; `order-42` appears in `url.path` only

#### Scenario: Evaluation keeps its name
- **WHEN** `POST /v1/evaluate` is served
- **THEN** the server span is still named `POST /v1/evaluate` with the child `discount.evaluate`

### Requirement: Health checks are not traced
`GET /healthz` SHALL produce no span, no trace and no HTTP request log record. All other
routes SHALL be traced as before.

#### Scenario: Health probe
- **WHEN** `GET /healthz` is served
- **THEN** no span is exported for the request and the response is still `200 {"status":"ok"}`

### Requirement: Server spans carry the response status
Every HTTP server span SHALL record `http.response.status_code` with the response status
code. For responses with status `5xx` the span status SHALL be `Error`; for `2xx` and `4xx`
(including `422` pricing rejections) the span status SHALL stay unset, per the OpenTelemetry
HTTP semantic conventions for servers.

#### Scenario: Pricing rejection
- **WHEN** an evaluation is rejected with `422` for `InvalidCurrency`
- **THEN** the server span has `http.response.status_code` `422`, its status is unset, and
  the child span carries `discount.error_type` `InvalidCurrency`

#### Scenario: Storage failure
- **WHEN** a request fails with `503` because the store is unavailable
- **THEN** the server span has `http.response.status_code` `503` and status `Error`

### Requirement: W3C trace context is honoured
The service SHALL extract W3C Trace Context (`traceparent`, `tracestate`) from incoming
request headers. When a valid `traceparent` is present the server span SHALL be a child of
the remote span and share its trace id; otherwise a new trace SHALL start. Export and
resource identity are unchanged.

#### Scenario: Caller sends traceparent
- **WHEN** `POST /v1/evaluate` arrives with
  `traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01`
- **THEN** the exported server span has trace id `4bf92f3577b34da6a3ce929d0e0e4736` and
  parent span id `00f067aa0ba902b7`, and `discount.evaluate` is in the same trace

#### Scenario: No traceparent
- **WHEN** a request arrives without trace headers
- **THEN** the server span is a root span of a freshly generated trace id
