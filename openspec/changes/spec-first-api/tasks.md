# Tasks

## 1. Contract

- [x] 1.1 Move the document to `api/openapi.yaml`, downgrade to OpenAPI 3.0.3, add operationIds; verify Redocly lint passes and the service still serves it at `/openapi.yaml`

## 2. Generated client (`src/client`)

- [x] 2.1 Add the `discount-client` crate with a progenitor build script; verify `cargo build -p discount-client`
- [x] 2.2 Contract test driving evaluate, redeem, confirm, get redemption and catalogue calls through the generated client against the running router; verify `cargo test -p discount-client`

## 3. Integration

- [x] 3.1 CI: Redocly lint of `api/openapi.yaml`; README: spec-first section with the generation commands for other languages; verify CI config is valid YAML and the README commands run

## Workflow follow-up

- Archive the change after review: `openspec archive spec-first-api`.
