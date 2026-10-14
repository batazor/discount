# Spec Delta

## ADDED Requirements

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
