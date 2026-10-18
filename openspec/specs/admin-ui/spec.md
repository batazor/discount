# admin-ui Specification

## Purpose
The operator console of the discount service: a web UI over the HTTP API for editing the
catalogue, inspecting redemptions and customers, and explaining a pricing result.

## Requirements

### Requirement: The UI is a client of the API contract
The console SHALL talk to the service only through the operations in `api/openapi.yaml`,
using the ubiquitous-language JSON shapes unchanged. It SHALL NOT compute a price, a coupon
status or a usage counter itself; every number it shows comes from a response. The base
URL of the service SHALL come from the environment variable `DISCOUNT_API_URL` (default
`http://localhost:8080`).

#### Scenario: Only documented operations
- **WHEN** the console's outgoing requests are compared with `api/openapi.yaml`
- **THEN** every request matches a documented path and method, with a documented body

#### Scenario: Service unreachable
- **WHEN** the service at `DISCOUNT_API_URL` does not answer
- **THEN** every page renders with a visible "service unreachable" notice naming the URL,
  and the console does not crash

### Requirement: Catalogue screens
The console SHALL provide screens for the pricing policy, stacking groups, promotions,
coupons, grant rules and exchange rates. Each list screen SHALL show every record the
corresponding `GET` returns; each record SHALL be editable in a form that submits the
record with `PUT` to its documented path, and deletable with `DELETE` where the API
offers it (groups, promotions, coupons, grant rules, exchange rates).

#### Scenario: Create a promotion
- **WHEN** an operator fills the promotion form with `promotion_id = P9`, group `g`,
  action `OrderPercentage 1000` and submits
- **THEN** the console sends `PUT /v1/promotions/P9` with that body and the promotions
  list shows `P9`

#### Scenario: Archive a promotion
- **WHEN** an operator deletes `P9` and confirms
- **THEN** the console sends `DELETE /v1/promotions/P9` and `P9` leaves the list

#### Scenario: Edit the policy
- **WHEN** an operator changes rounding to `HalfEven` and submits
- **THEN** the console sends `PUT /v1/policy` with the full `PricingPolicy` and the screen
  shows the new value

### Requirement: Rules and actions are edited structurally
The promotion form SHALL offer every `Rule` variant and every `Action` variant of the
contract as a choice with the fields of that variant, and SHALL submit them as the
tagged objects of the ubiquitous language. Money fields SHALL be entered as an integer
amount in minor units plus a currency; no decimal input exists.

#### Scenario: Add a date window rule
- **WHEN** an operator adds a rule, picks `DateWindow`, sets `starts_at` and leaves
  `ends_at` empty
- **THEN** the submitted promotion contains
  `{"type":"DateWindow","starts_at":<value>,"ends_at":null}`

#### Scenario: Fixed amount in minor units
- **WHEN** an operator picks `OrderFixedAmount` and enters amount `500`, currency `EUR`
- **THEN** the submitted action is
  `{"type":"OrderFixedAmount","amount":{"amount":500,"currency":"EUR"}}`

### Requirement: Derived fields are read-only
Fields the API derives (`uses_total` and `remaining_budget` on a promotion, `uses` on a
coupon) SHALL be displayed but not editable. The promotion screen SHALL edit the budget
through the budget resource, with an explicit "no budget" state for `null`.

#### Scenario: Budget set separately
- **WHEN** an operator sets `P1`'s budget to `1000000 EUR`
- **THEN** the console sends `PUT /v1/promotions/P1/budget` with
  `{"remaining":{"amount":1000000,"currency":"EUR"}}`, not a promotion `PUT`

#### Scenario: Uses are shown, not sent
- **WHEN** a coupon with `uses = 3` is opened and saved unchanged
- **THEN** the form shows `3` as read-only and the `PUT` body carries `uses = 0` or the
  displayed value, which the API ignores either way

### Requirement: Promotion metadata
The promotion screen SHALL show and edit the promotion's `PromotionMetadata` (title,
description, source, labels) through `GET|PUT|DELETE /v1/promotions/{promotion_id}/metadata`,
in a form separate from the promotion `PUT`; labels are entered one `key=value` per line.
A `404` on the metadata `GET` is the empty state, not an error. The promotions list and the
customer lookup SHALL show the title when metadata exists and SHALL offer a `source` filter
passed to the API as `?source=`.

#### Scenario: Metadata round trip from the screen
- **WHEN** an operator sets title `Summer`, source `hotels` and the label line `channel=app`
  on `P1` and saves
- **THEN** the console sends `PUT /v1/promotions/P1/metadata` with
  `{"title":"Summer","description":"","source":"hotels","labels":{"channel":"app"}}` and the
  promotions list shows `Summer` next to `P1`

#### Scenario: No metadata yet
- **WHEN** a promotion without metadata is opened
- **THEN** the metadata form is empty and no error is shown

#### Scenario: Filter by source
- **WHEN** an operator filters the promotions list by source `hotels`
- **THEN** the console sends `GET /v1/promotions?source=hotels` and lists only what the API
  returns

### Requirement: Coupon generation and grants
The coupons screen SHALL offer a generation form mapping onto
`POST /v1/coupons/generate` and SHALL show the generated codes once, ready to copy. The
grant rules screen SHALL offer a "grant now" form mapping onto `POST /v1/coupons/grant`.

#### Scenario: Generate a batch
- **WHEN** an operator requests `5` codes for `P3` with prefix `SPRING-` and `max_uses = 1`
- **THEN** the console shows the five codes returned by the API and the coupons list
  contains them

### Requirement: Redemption lookup
The console SHALL provide a lookup by `order_id` that renders the stored `Redemption`
(status, customer, the replayable input and the `PricingResult`) and offers confirm and
cancel actions mapped onto the documented transitions, showing the API's `409` body
when a transition is refused.

#### Scenario: Confirm from the screen
- **WHEN** an operator opens a reserved order and confirms it
- **THEN** the console sends `POST /v1/redemptions/{order_id}/confirm` and the screen
  shows status `confirmed`

#### Scenario: Refused transition
- **WHEN** an operator confirms an order that is already cancelled
- **THEN** the screen shows the `409` error body returned by the API next to the actions

### Requirement: Customer lookup
The console SHALL provide a lookup by `customer_id` that lists the customer's coupons
exactly as `GET /v1/customers/{customer_id}/coupons` returns them, with each
`CouponStatus` and, for `applied`, the `order_id` linked to the redemption lookup.

#### Scenario: Applied coupon links to its order
- **WHEN** customer `smoke` is looked up after the smoke run
- **THEN** the list shows the smoke code as `applied` with a link opening the redemption
  lookup for its `order_id`

### Requirement: Evaluate playground
The console SHALL provide a playground that builds an `EvaluateRequest` from a form (cart
lines, currency, tags, attributes, coupon codes, customer context) or from pasted JSON,
posts it to `POST /v1/evaluate` and renders the response: on `200` the `PricingResult`
with applied promotions, skipped promotions with their `SkipReason`, clamps with their
`ClampReason`, per-line results and allocations; on `422` the `PricingError` verbatim.
The playground SHALL never call `/v1/redeem`.

#### Scenario: Explain a skipped promotion
- **WHEN** the smoke catalogue is loaded and a cart below `P2`'s minimum is evaluated
- **THEN** the result panel lists `P2` under skipped with its `SkipReason` as returned

#### Scenario: Pricing error shown verbatim
- **WHEN** a cart with currency `eur` is evaluated
- **THEN** the panel shows `{"type":"InvalidCurrency","field":"cart.currency"}` and no
  totals

#### Scenario: Dry run only
- **WHEN** any playground evaluation completes
- **THEN** no redemption exists for it and no budget changed

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

### Requirement: Packaging and health
The console SHALL be a container image built from `src/ui`, listening on `:4321`, and
SHALL answer `GET /healthz` with `200` while it is serving pages regardless of the
service's state.

#### Scenario: Liveness
- **WHEN** `GET http://localhost:4321/healthz` is requested
- **THEN** the response is `200`

### Requirement: Every option explains itself
Each select of the console (rounding, stacking policy, stacking mode, rule type, action
type, grant trigger) SHALL show a one-sentence description of the chosen option beneath it,
worded after `docs/ubiquitous-language.md`, and SHALL update it when the choice changes.

#### Scenario: Stacking mode explained
- **WHEN** an operator picks `FirstMatch` on the group form
- **THEN** the text under the select says only the first eligible promotion of the group
  applies and the rest are skipped as `FirstMatchTaken`

### Requirement: Terms and badges carry their meaning
Terms that are not options (priority, exclusive, cap, budget, order cap, uses, coupon
binding and limits, grant validity, allocation) SHALL carry a one-sentence tooltip next to
their label or column, and status, mode, trigger, clamp and skip badges SHALL carry their
meaning as a tooltip; all worded after `docs/ubiquitous-language.md`.

#### Scenario: Priority says higher first
- **WHEN** the promotion or group form is opened
- **THEN** the priority field says higher applies first, as the vocabulary does

#### Scenario: Reasons explained in a result
- **WHEN** a result shows a promotion clamped by `Budget` or skipped as `ExclusiveApplied`
- **THEN** hovering the badge explains the reason

### Requirement: Promotion allowlist
The promotion screen SHALL show and replace the promotion's allowlist through
`GET|PUT|DELETE /v1/promotions/{promotion_id}/allowlist`, entered one customer id per line
and deduplicated before sending, and SHALL warn when the promotion has no
`RequiresAllowlist` rule, since the list then has no effect on pricing.

#### Scenario: Replace and clear
- **WHEN** an operator enters `c1`, `c2`, `c1` and saves, then clears the list
- **THEN** the console sends `PUT` with `{"customer_ids":["c1","c2"]}` and the card shows
  2, then sends `DELETE` and the card shows 0

### Requirement: Customer context in the playground
The playground form SHALL take the customer's attributes (key and value rows) and the
promotion ids the caller claims the customer is allowlisted for, and send them as
`customer.attributes` and `customer.allowlisted`.

#### Scenario: Allowlisted by the caller
- **WHEN** `P8` requires an allowlist and the playground lists `P8` under allowlisted
  promotions for customer `c9`
- **THEN** `P8` applies in the result
