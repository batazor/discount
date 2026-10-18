# Spec Delta

## MODIFIED Requirements

### Requirement: API errors are shown verbatim
When the API answers `400`, `404`, `409` or `422`, the console SHALL show the status and
the response body unchanged next to the form that caused it, keeping the operator's
input in place, together with the trace id the console sent on that request, linked to
the trace in Grafana Explore (Tempo) at `DISCOUNT_GRAFANA_URL`. The "service unreachable"
panel SHALL show the same trace id.

#### Scenario: Unknown key from a pasted body
- **WHEN** pasted promotion JSON contains an unknown key
- **THEN** the screen shows the `400` `decode_error` naming the key and the pasted text
  remains in the editor

#### Scenario: Trace id next to the error
- **WHEN** the API answers `422` to an evaluation
- **THEN** the panel shows `trace <id>` where `<id>` is the 32-hex trace id of the
  `traceparent` the console sent, and the id links to Grafana Explore with a Tempo query
  for that id
