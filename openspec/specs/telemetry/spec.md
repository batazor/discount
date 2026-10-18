# telemetry Specification

## Purpose
What the service emits over OpenTelemetry for every evaluation, with names stable enough
for dashboards and alerts to depend on. Telemetry is derived from the `PricingResult`; it
never changes what is priced.

## Requirements

### Requirement: One trace per evaluation
Each `POST /v1/evaluate` SHALL produce a server span `POST /v1/evaluate` with a child span
`discount.evaluate` carrying the attributes `discount.currency`, `discount.lines`
(line count), `discount.promotions` (count offered), `discount.applied` (count applied),
`discount.skipped` (count skipped), `discount.original_subtotal`,
`discount.total_discount` (minor units, integers) and, on error, `discount.error_type`
(the `PricingError` variant name). The span SHALL be exported over OTLP.

#### Scenario: Trace of a priced cart
- **WHEN** a valid cart with two applied and one skipped promotion is evaluated
- **THEN** a trace with the two spans exists in the trace backend and `discount.applied` is
  `2`, `discount.skipped` is `1`

### Requirement: Evaluation metrics
The service SHALL record, via OpenTelemetry metrics, exported over OTLP:
- `discount.evaluations` (counter) with attribute `outcome` in `ok | error` and, for
  errors, `error_type` (the `PricingError` variant);
- `discount.evaluate.duration` (histogram, seconds) with `outcome`;
- `discount.applied` (counter) with `promotion_id` and `clamped_by` in
  `none | Cap | Budget | OrderCap | Running`;
- `discount.applied.amount` (counter, minor units) with `promotion_id` and `currency`,
  the discount each applied promotion granted;
- `discount.skipped` (counter) with `promotion_id` and `reason` in
  `NotEligible | ExclusiveApplied | FirstMatchTaken`;
- `discount.amount` (histogram, minor units) with `currency`, recording `total_discount`
  of successful evaluations;
- `discount.share.bps` (histogram, basis points) recording
  `total_discount * 10000 / original_subtotal` for carts with a positive subtotal.

Attribute values SHALL reuse the ubiquitous-language names verbatim.

#### Scenario: Counters after one evaluation
- **WHEN** a cart is evaluated with promotions `P1` (applied, clamped by `Cap`) and `P3`
  (skipped, `NotEligible`)
- **THEN** `discount.evaluations{outcome="ok"}` increased by 1,
  `discount.applied{promotion_id="P1",clamped_by="Cap"}` by 1,
  `discount.applied.amount{promotion_id="P1"}` by the discount `P1` granted, and
  `discount.skipped{promotion_id="P3",reason="NotEligible"}` by 1

#### Scenario: Validation error metric
- **WHEN** a cart fails validation with `DuplicateLineId`
- **THEN** `discount.evaluations{outcome="error",error_type="DuplicateLineId"}` increased
  by 1 and no `discount.applied` sample was recorded

### Requirement: Logs correlated with traces
The service SHALL emit one structured log record per evaluation, the event
`discount.evaluation`, at `INFO` (success) or `WARN` (pricing error) containing the outcome
and `total_discount`, exported over OTLP logs with the active trace and span ids attached.

#### Scenario: Log carries trace id
- **WHEN** an evaluation completes
- **THEN** its log record in the log backend has the same trace id as the evaluation span

### Requirement: Resource identity
All signals SHALL carry the resource attributes `service.name=discount`,
`service.version` (crate version) and `deployment.environment` (from
`DISCOUNT_ENV`, default `local`).

#### Scenario: Service name in backends
- **WHEN** any signal is exported
- **THEN** it is queryable by `service.name="discount"` in Prometheus, Tempo and Loki

### Requirement: Telemetry never changes pricing
Instrumentation SHALL live outside `src/domain/` and `src/applicationlication/`; the `PricingResult` returned to the
caller SHALL be byte-for-byte the one computed before any telemetry is recorded. Exporter
failures SHALL not fail evaluations.

#### Scenario: Collector down
- **WHEN** the OTLP endpoint is unreachable
- **THEN** `POST /v1/evaluate` still returns the correct `PricingResult` and the service
  logs the export failure

### Requirement: Redemption metrics
The service SHALL record `discount.redemptions` (counter) with attributes `action` in
`reserve | confirm | cancel`, `outcome` in `ok | idempotent | rejected | error` and, for
`rejected`, `reason` in `NotFound | InvalidTransition | Pricing`; `error` SHALL mean a
storage failure only. It SHALL also record
`discount.budget.remaining` (observable gauge, minor units) with `promotion_id` and
`currency` for every promotion that has a budget, refreshed from the catalogue at least
every 15 seconds.

#### Scenario: Budget gauge after a reservation
- **WHEN** `P1` had budget 1000 and a redemption reserved 300
- **THEN** within 15 seconds `discount.budget.remaining{promotion_id="P1"}` reads 700

#### Scenario: Second confirm is a rejection, not an error
- **WHEN** an order is confirmed and then confirmed again
- **THEN** `discount.redemptions{action="confirm",outcome="ok"}` and
  `discount.redemptions{action="confirm",outcome="rejected",reason="InvalidTransition"}`
  each increased by 1 and `outcome="error"` did not change

#### Scenario: Storage failure is an error
- **WHEN** the redemption store is unavailable during a reserve
- **THEN** `discount.redemptions{action="reserve",outcome="error"}` increased by 1

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

### Requirement: Database spans
Every call into the Postgres repositories SHALL produce a span named `db.<method>` (the
repository method, for example `db.reserve`, `db.get_promotion`) of kind `client`, child of
the current span, with attributes `db.system.name` = `postgresql` and `db.operation.name` =
`<method>`. A call that fails with a storage error SHALL set the span status to `Error`. The
in-memory store SHALL produce no such spans.

#### Scenario: Redemption trace shows the transaction
- **WHEN** `POST /v1/redeem` reserves an order against Postgres
- **THEN** its trace contains `db.reserve` under the server span with
  `db.system.name=postgresql` and `db.operation.name=reserve`

#### Scenario: Catalogue read
- **WHEN** `GET /v1/policy` is served against Postgres
- **THEN** the trace contains `db.get_policy` of kind `client`

### Requirement: Console calls carry trace context
Every request the operator console sends to the service SHALL carry a W3C `traceparent`
header `00-<trace-id>-<parent-id>-01` with a trace id generated per request, and the
console's call outcome SHALL expose that trace id so a page can show it next to an error.

#### Scenario: Action request in Tempo
- **WHEN** the console posts an evaluation and the service exports its spans
- **THEN** Tempo has a trace with the console's trace id whose server span's parent id is
  the console's parent id

#### Scenario: Trace id on an error outcome
- **WHEN** the service answers `422`
- **THEN** the console's outcome carries `status` `422`, the verbatim body and the
  `traceId` it sent

### Requirement: The telemetry vocabulary is a Weaver registry
Every signal, attribute, unit and enum member the service emits SHALL be declared in the
OpenTelemetry Weaver registry `api/telemetry/` (ADR 12). `cargo xtask weaver check` SHALL pass,
a test SHALL fail when a name in `telemetry.rs` is missing from the registry, and
`cargo xtask live-check` (a running service exporting to Weaver) SHALL report no violation.

#### Scenario: A metric is renamed in code only
- **WHEN** `METRIC_APPLIED` in `telemetry.rs` is changed to `discount.granted`
- **THEN** `cargo test -p discount-infrastructure` fails naming the missing registry entry

#### Scenario: An undeclared attribute is emitted
- **WHEN** the service starts recording `discount.customer_id` on `discount.evaluate`
- **THEN** `cargo xtask live-check` exits non-zero with a `missing_attribute` violation for it

#### Scenario: Registry and service agree
- **WHEN** `cargo xtask live-check` runs against the current service
- **THEN** it exits 0; remaining findings are at most `improvement` (un-namespaced metric
  labels, the deprecated `deployment.environment`), never `violation`

#### Scenario: Bridge attributes and operational logs are outside the contract
- **WHEN** the tracing bridges add `code.*`, `thread.*`, `target`, `busy_ns`, `idle_ns` or
  `level`, or the service logs a record without an event name (start-up, budget refresh)
- **THEN** `cargo xtask live-check` reports nothing for them; the filters and their reasons are
  in `.weaver.toml`
