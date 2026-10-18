# 10. Spec-first HTTP API: OpenAPI is the contract, clients are generated

Date: 2026-10-15

## Status

Accepted

## Context

The HTTP adapter was written by hand and documented afterwards in the README. Consumers of
the service (checkout, admin tooling, the promocode service it replaces) need a contract
they can generate clients from, and the contract must not drift from the server. The
pricing engine already has this discipline (Lean model first, Rust mirror checked by DRT);
the API edge should have the same shape: specification first, implementation checked
against it.

Options considered:

1. Generate the server from the spec (openapi-generator `rust-axum`). Rejected for now:
   the generated server traits constrain the adapter design and the generator's Rust
   output lags axum; the adapter is small enough to keep by hand under a contract test.
2. Generate the spec from the server (`utoipa` derives). Rejected: it puts the contract
   second and would require the derive on domain types, which must stay std + serde (ADR 8).
3. Hand-written OpenAPI document as the source, clients generated from it, server checked
   against it. Chosen.

## Decision

- `api/openapi.yaml` is the HTTP contract (OpenAPI 3.0.3 so that every mainstream
  generator reads it; every operation has an `operationId`). A change to the API starts
  there, then in `openspec/specs/` scenarios, then in code.
- JSON shapes in the document are the ubiquitous language; the document never introduces a
  wire format of its own.
- `src/client` (`discount-client`) is generated from the document at build time with
  progenitor; nothing in it is written by hand. Clients for other languages are generated
  from the same file with openapi-generator (command in the README); they are not
  committed.
- The service serves the document at `/openapi.yaml` and Swagger UI at `/docs`, embedded
  from the same file, so what a running instance shows is what the repository holds.
- The operator console (`src/ui`, Astro) is a second generated consumer: its TypeScript
  types come from the document at build time (`openapi-typescript`) and its requests go
  through a typed client (`openapi-fetch`), so a call outside the contract does not
  compile. It adds no operation of its own and computes nothing the service computes.
- Contract tests: `src/infrastructure` checks that every router path is in the document;
  `src/client/tests` starts the service and drives the main flows through the generated
  client, so a server response that does not match the schema fails the build. CI lints
  the document with Redocly.

## Consequences

- Two sources of truth about behaviour coexist on purpose: `openspec/specs/` says what
  the service does (scenarios), `api/openapi.yaml` says how it is called (shapes and
  status codes). The ubiquitous language binds them.
- Adding a field is a three-step change (spec, OpenSpec scenario, code) and the generated
  client picks it up on the next build without edits.
- OpenAPI 3.0 cannot express a few 3.1 niceties (`const`, type arrays); nullable fields use
  `nullable: true` and discriminated unions use `oneOf` with single-value `enum`s.
