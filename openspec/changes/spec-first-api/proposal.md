# Proposal

## Why

Consumers need a generated client and a contract that cannot drift from the server; today
the API is documented after the fact. ADR 10 makes `api/openapi.yaml` the contract.

## What Changes

- `api/openapi.yaml` (OpenAPI 3.0.3, operationIds) is the HTTP contract and is served by
  the service at `/openapi.yaml` with Swagger UI at `/docs`.
- A generated Rust client crate `src/client` (progenitor, build time) and contract tests
  that drive the service through it.
- CI lints the document; the README points at the spec and the generation commands.

Outside the verified scope: all of it.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `evaluate-http-api`: the API is defined by the OpenAPI document; a generated client must
  be able to drive every documented flow.

## Impact

`api/`, `src/client/`, `src/infrastructure/src/http.rs` (serving the document),
`.github/workflows/ci.yml`, `README.md`, `Dockerfile`, ADR 10.
