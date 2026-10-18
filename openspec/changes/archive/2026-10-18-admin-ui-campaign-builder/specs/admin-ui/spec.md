# Spec Delta

## ADDED Requirements

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
