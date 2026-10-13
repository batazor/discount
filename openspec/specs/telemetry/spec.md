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
The service SHALL emit one structured log record per evaluation at `INFO` (success) or
`WARN` (pricing error) containing the outcome and `total_discount`, exported over OTLP logs
with the active trace and span ids attached.

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
Instrumentation SHALL live outside `core/` and `app/`; the `PricingResult` returned to the
caller SHALL be byte-for-byte the one computed before any telemetry is recorded. Exporter
failures SHALL not fail evaluations.

#### Scenario: Collector down
- **WHEN** the OTLP endpoint is unreachable
- **THEN** `POST /v1/evaluate` still returns the correct `PricingResult` and the service
  logs the export failure
