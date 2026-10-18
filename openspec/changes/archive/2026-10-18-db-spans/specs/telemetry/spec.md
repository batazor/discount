# Spec Delta

## ADDED Requirements

### Requirement: Database spans
Every call into the Postgres repositories SHALL produce a span named `db.<method>` (the
repository method, for example `db.reserve`, `db.get_promotion`) of kind `client`, child of
the current span, with attributes `db.system` = `postgresql` and `db.operation.name` =
`<method>`. A call that fails with a storage error SHALL set the span status to `Error`. The
in-memory store SHALL produce no such spans.

#### Scenario: Redemption trace shows the transaction
- **WHEN** `POST /v1/redeem` reserves an order against Postgres
- **THEN** its trace contains `db.reserve` under the server span with
  `db.system=postgresql` and `db.operation.name=reserve`

#### Scenario: Catalogue read
- **WHEN** `GET /v1/policy` is served against Postgres
- **THEN** the trace contains `db.get_policy` of kind `client`
