# Spec Delta

## ADDED Requirements

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
