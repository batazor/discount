# admin-ui Specification

## Purpose
The operator console of the discount service: a web UI over the HTTP API for editing the
catalogue, inspecting redemptions and customers, and explaining a pricing result.

## Requirements

### Requirement: The UI is a client of the API contract
The console SHALL talk to the service only through the operations in `api/openapi.yaml`,
using the ubiquitous-language JSON shapes unchanged. It SHALL NOT compute a price, a coupon
status or a usage counter itself; every number it shows comes from a response. A campaign
status it shows is a display classification of returned fields (see "Campaign status"),
never a pricing result. The base URL of the service SHALL come from the environment
variable `DISCOUNT_API_URL` (default `http://localhost:8080`).

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
The campaign form SHALL offer every `Rule` variant and every `Action` variant of the
contract as a choice with the fields of that variant, under plain names, and SHALL submit
them as the tagged objects of the ubiquitous language. Money SHALL be entered as a decimal
amount in the currency's units and converted to integer minor units by the currency's
ISO 4217 exponent; percentages SHALL be entered as percent and converted to basis points.
The integer that will be sent SHALL be shown next to the input.

#### Scenario: Add a date window rule
- **WHEN** an operator adds a rule, picks `DateWindow`, sets `starts_at` and leaves
  `ends_at` empty
- **THEN** the submitted promotion contains
  `{"type":"DateWindow","starts_at":<value>,"ends_at":null}`

#### Scenario: Fixed amount in minor units
- **WHEN** an operator picks a fixed amount and enters `5.00`, currency `EUR`
- **THEN** the submitted action is
  `{"type":"OrderFixedAmount","amount":{"amount":500,"currency":"EUR"}}`

#### Scenario: Currency without decimals
- **WHEN** an operator enters `500`, currency `JPY`
- **THEN** the submitted amount is `500`, and `500.5 JPY` is refused as a field error

#### Scenario: Percent to basis points
- **WHEN** an operator enters `12.5` as the percentage
- **THEN** the submitted action is `{"type":"OrderPercentage","bps":1250}`, and `12.345`
  is refused as a field error

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
The console SHALL provide an explain screen that builds an `EvaluateRequest` from a stored
redemption (by `order_id`, loading its replayable input), from a form (cart lines,
currency, tags, attributes, coupon codes, customer context) or from pasted JSON, posts it
to `POST /v1/evaluate` and renders the response: on `200` the `PricingResult` with applied
promotions, skipped promotions with their `SkipReason`, clamps with their `ClampReason`,
per-line results and allocations; on `422` the `PricingError` verbatim. The screen SHALL
never call `/v1/redeem`.

#### Scenario: Explain a stored order
- **WHEN** an operator enters the order id of a stored redemption
- **THEN** the form is filled from that redemption's input and the result panel shows the
  evaluation of that input as the service returns it now

#### Scenario: Explain a skipped promotion
- **WHEN** the smoke catalogue is loaded and a cart below `P2`'s minimum is evaluated
- **THEN** the result panel lists `P2` under skipped with its `SkipReason` as returned

#### Scenario: Pricing error shown verbatim
- **WHEN** a cart with currency `eur` is evaluated
- **THEN** the panel shows `{"type":"InvalidCurrency","field":"cart.currency"}` and no
  totals

#### Scenario: Dry run only
- **WHEN** any explain evaluation completes
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

### Requirement: Campaign status
The console SHALL classify each non-archived promotion from returned fields and the
current time as `scheduled` (a `DateWindow` rule with `starts_at` in the future),
`ended` (a `DateWindow` rule with `ends_at` in the past), `exhausted` (`remaining_budget`
of `0`, or a `MaxUsesTotal` rule with `uses_total ≥ max`) or `live` (otherwise), in that
precedence, and SHALL show it as a badge with its meaning.

#### Scenario: Exhausted by budget
- **WHEN** `P1`'s remaining budget is `0 EUR`
- **THEN** `P1` is listed as `exhausted`

#### Scenario: Ended wins over exhausted
- **WHEN** a promotion's window ended yesterday and its budget is `0`
- **THEN** it is listed as `ended`

#### Scenario: No window, budget left
- **WHEN** a promotion has no `DateWindow` rule and a positive budget
- **THEN** it is listed as `live`

### Requirement: Campaign screens
The console SHALL present a promotion with its budget, metadata, allowlist and coupons as
one campaign: a list with status, title, what it gives, when it runs, remaining budget and
number of codes; and a campaign page with tabs for overview, rules and action, codes,
audience (allowlist) and details (metadata). `/promotions` and `/promotions/{id}` SHALL
redirect to the campaign routes.

#### Scenario: Codes of a campaign
- **WHEN** `P3` has two stored coupons
- **THEN** the list shows `2` codes for `P3` and the codes tab lists both with a
  generation form for `P3` only

#### Scenario: Old link
- **WHEN** `/promotions/P1` is opened
- **THEN** the browser lands on the campaign page of `P1`

### Requirement: Creating a campaign in one flow
The console SHALL create a campaign from one form (title, description, source, what it
gives, rules, budget, codes to generate) by calling, in order, the promotion `PUT`, the
budget `PUT` when a budget was given, the metadata `PUT` when a title was given, and
coupon generation when codes were requested. The group SHALL be preselected when exactly
one exists.

#### Scenario: Full creation
- **WHEN** an operator creates `SUMMER` giving `10 %`, budget `1000.00 EUR`, title
  `Summer sale` and `5` codes with prefix `SUM-`
- **THEN** the service holds promotion `SUMMER` with `bps = 1000`, budget `100000 EUR`,
  the metadata and five `SUM-` coupons, and the browser lands on the campaign page
  showing the codes once

#### Scenario: Single group preselected
- **WHEN** the catalogue has exactly one group
- **THEN** the creation form opens with that group chosen

### Requirement: A failed creation step is reported
When a step of the creation sequence fails, the console SHALL name the step, show the
API's response verbatim, state which earlier steps were saved, and keep the form values.

#### Scenario: Budget step fails
- **WHEN** the promotion `PUT` succeeds and the budget `PUT` answers `400`
- **THEN** the screen says the promotion was saved, shows the `400` body for the budget
  step, and keeps the form values

### Requirement: Human units
Every amount SHALL be shown as a decimal in the currency's units with the integer minor
units in a tooltip; every basis-point value as a percentage; every timestamp in the
viewer's local time with the UTC value and the integer in a tooltip. Currencies without
an entry in the console's exponent table SHALL use two decimals.

#### Scenario: Three-decimal currency
- **WHEN** a budget of `1500 KWD` minor units is shown
- **THEN** the screen reads `1.500 KWD`

#### Scenario: Local time
- **WHEN** a coupon ends at Unix `1760313600`
- **THEN** the list shows that instant in the viewer's time zone and the tooltip carries
  the UTC time and `1760313600`

### Requirement: Find
The console SHALL offer one search box that takes a customer id, an order id, a coupon
code or a campaign id, looks each up with the documented `GET`, and shows what matched
with links to the matching screens.

#### Scenario: Order id
- **WHEN** `smoke-1` is entered and a redemption with that id exists
- **THEN** the result shows the redemption's status with links to its page and to
  explaining it

#### Scenario: Nothing matches
- **WHEN** `nope` matches nothing
- **THEN** the screen says so and lists what was looked up

### Requirement: List filters
Campaign and coupon lists SHALL offer a filter box applied in the browser to the visible
columns, and the campaign list status chips; without scripts the full lists remain.

#### Scenario: Filter coupons by prefix
- **WHEN** `SUM-` is typed in the coupon filter
- **THEN** only codes containing `SUM-` stay visible and the count updates

### Requirement: Destructive actions explain themselves
Archiving a campaign, deleting a coupon, group, grant rule or rate, cancelling a
redemption, clearing an allowlist and removing metadata SHALL open a dialog that states
the consequence in plain words (for archive: codes stop working, existing redemptions keep
their copy) and requires a second click; without scripts the action still submits after
the browser's own confirmation.

#### Scenario: Archive dialog
- **WHEN** an operator clicks Archive on a campaign
- **THEN** a dialog names the campaign, states that its codes stop working and existing
  redemptions keep their copy, and the archive happens only on its confirm button

### Requirement: Generated codes can be taken away
After a generation the console SHALL offer copy-all and a CSV download of the codes shown.

#### Scenario: CSV
- **WHEN** five codes were generated
- **THEN** the download is a file with one code per line

### Requirement: Exchange-rate source screen
The console SHALL provide a screen Settings → Exchange rates → Source with a form for the
URL and, in human units (minutes, hours or days, sent as seconds), the fetch interval, the
maximum age before rates count as stale and the fetch timeout, submitted with
`PUT /v1/exchange-rate-source`. It SHALL show the status as the service reports it (last
successful fetch and number of pairs, next due time, the last error verbatim, a `stale`
badge) and offer Try again (`POST /v1/exchange-rate-source/refresh`, showing the `502`
message verbatim) and Turn off (`DELETE`, a destructive action that explains that stored
rates are kept and become manual). The help text SHALL say that a key in the URL is visible
to every operator and that pricing keeps the last stored rates while fetches fail.

#### Scenario: Configure from the console
- **WHEN** an operator enters a URL, chooses "every 1 hour", "stale after 1 day",
  "timeout 10 seconds" and saves
- **THEN** the console sends `PUT /v1/exchange-rate-source {url, interval_seconds: 3600,
  max_age_seconds: 86400, timeout_seconds: 10}` and the screen shows "next fetch" from the
  returned status

#### Scenario: Try again fails
- **WHEN** the operator clicks Try again and the provider is down
- **THEN** the screen shows the service's `502` message verbatim and the rate list is
  unchanged

### Requirement: Source status on the exchange-rates screen
The exchange-rates screen SHALL link to the source screen and, while a source is set, show
its status in one line (last fetch, stale badge, last error verbatim) with a Try again
button, and SHALL show on every rate row its origin (`manual`, or `source` with
`fetched_at`).

#### Scenario: Origin on rows
- **WHEN** the source stored `EUR → USD` and an operator set `USD → EUR` by hand
- **THEN** the first row shows `source` with its fetch time and the second shows `manual`

### Requirement: Campaign wizard
`/campaigns/new` SHALL lead through steps (kind, what it gives, who and when, budget and
codes, name and review) over a single form, with next and back that do not lose values;
without scripts every step SHALL be visible at once. The plain form SHALL remain at
`/campaigns/new/form`. Submitting either SHALL create the campaign as the creation flow
specifies.

#### Scenario: Steps keep values
- **WHEN** an operator enters `10` percent on the second step, goes to the third and back
- **THEN** the percentage is still `10`

#### Scenario: Without scripts
- **WHEN** the wizard is rendered without scripts
- **THEN** every step's fields are visible and one submit creates the campaign

### Requirement: Live summary
The wizard SHALL show the campaign as one sentence built from the current fields (what
it gives, for whom and when, cap, budget, codes), updated as the operator types, and the
same sentence on the review step without scripts.

#### Scenario: Sentence
- **WHEN** the fields say `10` percent, first order only, dates 1–30 November, budget
  `1000.00 EUR`, `500` codes with one use each
- **THEN** the summary reads `10 % off the order, for first orders only, from <1 Nov> to
  <30 Nov>, up to 1 000.00 EUR in total, with 500 single-use codes`

### Requirement: Campaign templates
The wizard's first step SHALL offer templates (everyone, welcome, code-only, date-bound
sale, VIP allowlist, win-back) that prefill the fields through the URL; a template SHALL
be a starting point, every field remaining editable.

#### Scenario: Welcome template
- **WHEN** `/campaigns/new?template=welcome` is opened
- **THEN** the rule "first order only" and "once per customer" are on, the action is a
  percentage, and nothing is created until submit

### Requirement: Rule builder in plain words
The wizard's "who and when" step SHALL offer named conditions (dates, minimum order
total, first order only, once per customer, needs a code, only allowlisted customers,
cart tags, customer attribute, limit total uses), each a toggle revealing its inputs, and
SHALL submit them as the contract's `Rule` variants; the raw rule editor SHALL stay on the
campaign's rules tab.

#### Scenario: First order and dates
- **WHEN** "first order only" and "dates" with a start are on
- **THEN** the created promotion's rules are `MaxCompletedOrders { max: 0 }` and
  `DateWindow { starts_at: <start>, ends_at: null }`

#### Scenario: Needs a code generates codes
- **WHEN** "needs a code" is on and `5` codes are requested
- **THEN** the promotion carries `RequiresCoupon` and five codes exist after creation

### Requirement: Try it on a sample cart
The campaign page SHALL offer a sample cart (currency, lines, optional coupon code,
customer id, completed orders and customer attributes), evaluate it with
`POST /v1/evaluate` and show for this campaign whether it applied with the discount
granted, or why it was skipped, with the full result available; nothing is stored.

#### Scenario: Applied
- **WHEN** campaign `SUMMER` gives `10 %` to every cart and a `100.00 EUR` cart is tried
- **THEN** the card says `SUMMER` applied and granted `10.00 EUR`

#### Scenario: Skipped with the reason
- **WHEN** `SUMMER` requires a code and the sample cart presents none
- **THEN** the card says `SUMMER` was skipped as `NotEligible` naming the "needs a code"
  rule

### Requirement: Templates gallery and duplication
`/campaigns/templates` SHALL list every template with its sentence and link into the
wizard, and SHALL list existing campaigns to start from. `/campaigns/new?from=<id>` SHALL
prefill the wizard from that campaign's action, cap, conditions, budget currency, title,
source and labels, with an empty id, and SHALL create nothing until submit.

#### Scenario: Duplicate
- **WHEN** `SUMMER` gives `10 %` to first orders with a code and the wizard is opened with
  `from=SUMMER`
- **THEN** the percentage is `10`, "first order only" and "needs a code" are on, the id
  is empty and the title reads `SUMMER` copy

### Requirement: Rule builder on the rules tab
The campaign's rules tab SHALL show the builder's conditions prefilled from the stored
rules; rules the builder cannot express SHALL appear as advanced rules in the raw editor
beside it; saving SHALL send the builder's rules followed by the advanced ones.

#### Scenario: Round trip
- **WHEN** a campaign has `MaxCompletedOrders 0`, `RequiresCoupon` and a `TagsMatch` with
  `none = ["#partner"]`
- **THEN** the tab shows "first order only" and "needs a code" on, the tag rule as an
  advanced rule, and saving unchanged stores the same three rules

### Requirement: More conditions
The builder SHALL also offer cart attribute (`AttributeIn`), cart date attribute
(`AttributeDateWindow`), customer attribute range (`CustomerAttributeRange`) and maximum
order total (`OrderTotalRange` with `max`); minimum and maximum order total SHALL become one
`OrderTotalRange` when both are on.

#### Scenario: Both bounds
- **WHEN** minimum `50.00 EUR` and maximum `500.00 EUR` are on
- **THEN** one rule `OrderTotalRange { min: 5000 EUR, max: 50000 EUR }` is sent

### Requirement: Builder sanity checks
Before sending, the builder SHALL refuse an end before its start and a minimum above the
maximum as field errors, and SHALL warn on the campaign page when "needs a code" is on
and no code exists.

#### Scenario: End before start
- **WHEN** dates are on with an end before the start
- **THEN** the end field shows an error and nothing is sent

### Requirement: Who gets it
The rules tab and the wizard SHALL show the conditions as one sentence, updated as they
change.

#### Scenario: Sentence of conditions
- **WHEN** "first order only" and "needs a code" are on
- **THEN** the sentence reads `for first orders only, with a code`

### Requirement: Sign-in gate
When `DISCOUNT_UI_OIDC_ISSUER` is set, every console page except `/login`, `/healthz`,
`/api-docs` and `/openapi.yaml` SHALL require a signed-in session and SHALL redirect to
`/login` with the requested path kept. Without the variable the console SHALL run without
any sign-in, as before.

#### Scenario: Redirected to sign in
- **WHEN** `/campaigns` is opened without a session
- **THEN** the browser lands on `/login?back=/campaigns`

### Requirement: Signing in
`/login` SHALL obtain tokens from the issuer with the submitted username and password (or,
with `DISCOUNT_UI_OIDC_FLOW=code`, through the issuer's authorization endpoint) and store
them in the server-side session only; the browser SHALL receive nothing but the session
cookie.

#### Scenario: Wrong password
- **WHEN** a wrong password is submitted
- **THEN** the page shows the issuer's error and keeps the username

#### Scenario: Signed in and back
- **WHEN** `marketing` signs in from `/login?back=/campaigns`
- **THEN** the browser lands on `/campaigns` and the page lists the campaigns of the
  user's tenant

### Requirement: Token on every call
Every request the console makes to the service SHALL carry the session's access token as
`Authorization: Bearer`; a `401` from the service SHALL be shown with a link to sign in
again, a `403` verbatim as any API error.

#### Scenario: Support cannot write the catalogue
- **WHEN** `support` saves a stacking group
- **THEN** the form shows the service's `403` problem body

### Requirement: Session refresh and sign-out
A token expiring within sixty seconds SHALL be refreshed with the refresh token before a
page renders; when the refresh fails the session SHALL be cleared and the operator sent to
`/login`. Sign-out SHALL clear the session and revoke the refresh token when the issuer
offers revocation.

#### Scenario: Sign out
- **WHEN** the operator signs out
- **THEN** the browser lands on `/login` and opening `/campaigns` redirects there again

### Requirement: Who is signed in
The navigation SHALL show the signed-in name, tenant and scopes with a sign-out control,
and the role SHALL default from the scopes (catalogue write → `marketing`, otherwise
`support`) until the operator switches it.

#### Scenario: Support role by default
- **WHEN** `support` signs in with no role cookie
- **THEN** the overview is the support overview

### Requirement: Demo users in the local realm
The Keycloak realm of the auth overlay SHALL contain a public client `discount-console`
with direct access grants, tenant `hotels`, and the users `marketing` (catalogue read and
write, pricing, customers) and `support` (pricing, customers, catalogue read); the overlay
SHALL point the console at the realm and name the demo users on the sign-in page.

#### Scenario: Demo users listed
- **WHEN** the auth overlay is up and `/login` is opened
- **THEN** the page names `marketing` and `support` as demo users

### Requirement: Tenant visible
The console SHALL show the current tenant at the top of the navigation, in the overview
heading and in the description of the campaign, coupon and customer screens: the token's
tenant when signed in, `default` when the token names none, `local` without sign-in. The
console SHALL NOT offer tenant switching and SHALL say so next to `default`.

#### Scenario: Signed in to hotels
- **WHEN** `marketing` (tenant `hotels`) opens the campaigns list
- **THEN** the navigation and the description name `hotels`

#### Scenario: No sign-in
- **WHEN** the console runs without an issuer
- **THEN** the navigation names `local`

### Requirement: Navigation follows scopes
The navigation SHALL lead with the campaign screens when the token carries
`discount:catalog.write` (or `discount:admin`) and with the support screens otherwise, and
SHALL leave out screens whose scope the token lacks: campaigns, coupons and settings need
`discount:catalog.read`, Explain and redemptions `discount:pricing`, customers
`discount:customers`, Find either of the last two. Without sign-in every screen SHALL be
offered.

#### Scenario: Support account
- **WHEN** `support` (pricing, customers, catalogue read) signs in
- **THEN** the navigation leads with Find and Explain, lists campaigns and settings after,
  and offers no role switch

### Requirement: Overview
The overview SHALL show the find box and recent lookups when the token can look customers
or orders up, the campaign health sections (live, ending within seven days, lowest
remaining budgets, exhausted, scheduled) when it can read the catalogue, and the first-run
checklist when it can write the catalogue and the catalogue has no group or no campaign.

#### Scenario: First run
- **WHEN** the catalogue has no groups and the token can write it
- **THEN** the overview shows the checklist with "create a group" as the next step

#### Scenario: Ending soon
- **WHEN** a campaign's window ends in three days
- **THEN** it is listed under "ending soon" with the end shown in local time

### Requirement: Actions follow scopes
Without `discount:catalog.write` the console SHALL render catalogue forms read-only with a
note and SHALL NOT show create, save, archive, delete or generate controls; without
`discount:pricing` it SHALL NOT offer Explain, "try it" or the redemption transitions;
without `discount:customers` it SHALL NOT offer the customer screens. A direct request
without the scope SHALL still show the service's `403` verbatim.

#### Scenario: Support sees the catalogue read-only
- **WHEN** `support` opens a campaign's rules tab
- **THEN** the fields are disabled, a read-only note is shown and there is no Save

#### Scenario: Budget edited from the overview
- **WHEN** `marketing` sets a campaign's budget on its overview tab
- **THEN** the console sends the budget `PUT` and the remaining budget updates
