# 13. Telemetry schema as code: the Weaver registry is the contract, live-checked against the service

Date: 2026-10-18

## Status

Accepted

## Context

The service's telemetry (resource, HTTP and `discount.evaluate` spans, `db.*` spans, nine
metrics, one log record per evaluation) is the contract the Grafana dashboards and alert
rules depend on. Until now that contract lived in three prose places: the "Telemetry
vocabulary" section of the ubiquitous language, the `telemetry` spec and the constants in
`src/infrastructure/src/telemetry.rs`, with one test checking that the constants appear in
the prose. Nothing checked types, units, enum members or which attributes a signal carries,
and nothing checked what the running service actually emits: the first live check found
`usize` log fields exported as strings, log records with no event name, a `db.system`
attribute that upstream had renamed, and a dozen attributes added by the tracing bridges
that no document mentioned.

The HTTP edge already has the discipline we want (ADR 10: the OpenAPI document is the
contract, the server is checked against it). OpenTelemetry Weaver gives the same shape to
telemetry: a semantic-convention registry in the upstream YAML format, validated
(`registry check`), diffed between versions (`registry diff`), and compared with real OTLP
traffic (`registry live-check`).

Options considered:

1. Keep the prose and the constants test. Rejected: it cannot catch a type, unit or
   attribute-set drift, and a renamed attribute surfaces only when a dashboard breaks.
2. Generate the Rust constants from the registry (`registry generate` with a Jinja
   template). Deferred: the constants are twenty lines; a test that every constant is in
   the registry costs nothing and keeps the build free of Docker. Codegen can come when
   there is a second consumer (TypeScript console, collector config).
3. A hand-written registry that depends on the upstream conventions, checked by a test
   against the constants and by live-check against the running service. Chosen.

## Decision

- `api/telemetry/` is the telemetry contract: a Weaver registry (`manifest.yaml`,
  `discount.yaml`, `file_format: definition/2`) that depends on the upstream semantic
  conventions `v1.44.0` and references their attributes (`http.*`, `url.path`, `db.*`,
  `service.*`) rather than redefining them. It declares every signal the service emits
  with its kind, unit, attributes, requirement levels and enum members, and nothing the
  service does not emit.
- The registry is the single list of names: the ubiquitous language keeps the human
  summary and `telemetry.rs` the constants; `registry_declares_every_name` fails the build
  when a constant is missing from the registry, `ubiquitous_language_lists_every_name`
  when it is missing from the prose.
- Weaver runs from the `otel/weaver` image, never installed on the host:
  `cargo xtask weaver <subcommand>` for `check`, `resolve`, `diff`, `search`;
  `cargo xtask live-check` starts Weaver's OTLP listener, runs `discount-server` against it
  with the smoke traffic, stops the service so it flushes, and fails on any `violation`.
  Both are part of the local verification before a release (there is no CI).
- `.weaver.toml` holds what describes the emitter rather than the schema: the matchers
  that pair free-form span names with their definitions, and the filters. Only two kinds
  of finding are filtered, each with its reason in the file: attributes the tracing bridges
  add to every span and log record (`code.*`, `thread.*`, `target`, `busy_ns`, `idle_ns`,
  `level`; declared in the `discount.tracing_bridge` group so they are permitted but never
  required), and log records without an event name, which are operational (start-up,
  budget refresh) and not contract. The deprecated `deployment.environment` is downgraded
  to `improvement`, not hidden, until the dashboards move with it.
- Metric labels keep their un-namespaced ubiquitous-language names (`outcome`,
  `promotion_id`, `reason`, ...): the dashboards and alert rules query them, and Weaver's
  namespace advice is an `improvement`, not a violation. Renaming them is a contract change
  with a dashboard migration, decided separately.
- A contract change is made in the registry first, then in the code, then in the
  dashboards; `cargo xtask weaver diff --baseline-registry <previous>` lists what changed.

## Consequences

- A new signal or attribute that is not in the registry fails `cargo test` (if it is a
  constant) or `cargo xtask live-check` (if it reaches OTLP). The `discount.evaluation` log
  record now has an event name and integer counts; `db.*` spans carry `db.system.name`.
- The registry documents the debt it found: `deployment.environment` (renamed upstream,
  37 label uses in Grafana) and the un-namespaced metric labels. Both are visible in
  every live-check report as improvements.
- `cargo xtask live-check` needs Docker and about thirty seconds (one budget-gauge refresh); it
  exercises the in-memory store unless `DATABASE_URL` is set, so `db.*` spans are checked
  only against Postgres.
- Weaver's v2 file format is marked "not yet stable" by the tool; the registry is pinned
  to `otel/weaver:v0.28.0` and semantic conventions `v1.44.0`, and bumping either is a
  deliberate change checked by `cargo xtask weaver check`.
