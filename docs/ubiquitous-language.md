# Ubiquitous language

The single list of names used in `verification/spec/` (Lean 4), `src/domain/` (Rust), `docs/invariants.md`
and in conversation. A name here is spelled identically in both code bases (modulo
language casing: `CartSnapshot` in both, `lineSubtotal` in Lean / `line_subtotal` in Rust).
Adding a concept means adding it here first (ADR 8).

JSON encodings are normative: they are the DRT contract between the Lean reference and
the Rust core (ADR 2). Rules shared by every type:

- Objects carry exactly the listed keys. Extra keys on input are an error at decode time,
  not a `PricingError`. Key order on output is not significant (the Lean reference emits
  keys alphabetically); the DRT compares parsed values.
- Enums encode as an object with a `"type"` key holding the variant name, followed by the
  variant's fields: `{"type": "HalfUp"}`, `{"type": "OrderPercentage", "bps": 1500}`.
- Optional fields encode as `null` when absent. A missing key is read as `null`.
- Integers are JSON numbers without fraction or exponent, within `i64`.
- Lists keep their order; order is semantically significant for `lines` (ADR 5).

## Constants

| Name | Value | Meaning |
|------|-------|---------|
| `MAX_MINOR_UNITS` | `10^14` | Upper bound for every amount and for the order subtotal (ADR 3). |
| `MAX_QUANTITY` | `10^9` | Upper bound for `quantity`. Together with the subtotal bound this keeps every intermediate within `i128`. |
| `BPS_SCALE` | `10_000` | Denominator of `BasisPoints`. |
| `DEFAULT_ROUNDING` | `Down` | What the application layer puts into `PricingPolicy.rounding` when nothing is configured (ADR 3). The core has no defaults. |
| `DEFAULT_STACKING` | `Sequential` | Same, for `PricingPolicy.stacking` (ADR 4). |

## Value types

### Currency
ISO 4217 alphabetic code: exactly three ASCII uppercase letters. The core knows nothing
else about a currency (no exponent, no symbol).
JSON: string, `"EUR"`.

### Money
An amount in **minor units** of a currency. `amount` is a signed 64-bit integer (`Int` in
Lean, `i64` in Rust); inside the engine every amount is in `0 ..= MAX_MINOR_UNITS`.
Fields: `amount`, `currency`.
JSON: `{"amount": 1999, "currency": "EUR"}`.

### BasisPoints
A percentage in hundredths of a percent: `0 ..= BPS_SCALE`. `1500` is 15 %.
JSON: number.

### Timestamp
An instant as Unix seconds (signed 64-bit). The core has no time zones (ADR 7).
JSON: number.

### LineId, GroupId, PromotionId
Opaque identifiers, each unique within its list. `GroupId` and `PromotionId` are totally
ordered by bytewise string comparison; that order is part of the stacking semantics (ADR 4).
JSON: string.

### Quantity
Integer in `1 ..= MAX_QUANTITY`.
JSON: number.

### Attribute
A string key and value describing the order (`"hotel_kind": "resort"`, `"checkin": "1760227200"`).
Fields: `key`, `value`.
JSON: `{"key": "checkin", "value": "1760227200"}`.

### Coupon
A code that turns a promotion on. Fields: `code`, `promotion_id`,
`starts_at : Option Timestamp`, `ends_at : Option Timestamp` (half-open window on `now`),
`customer_id : Option String` (the only customer allowed to use it),
`max_uses : Option Int` (absent = unlimited, `1` = single use), `uses : Int` (`≥ 0`, how many
reserved or confirmed redemptions already used the code; a snapshot counter the application
layer derives, like `remaining_budget`). Pricing receives the coupons whose code the cart
presents in `PricingInput.coupons`.
JSON: `{"code": "WELCOME", "promotion_id": "P1", "starts_at": null, "ends_at": null, "customer_id": null, "max_uses": 1, "uses": 0}`.

### ExchangeRate
One minor unit of `from` is `numerator / denominator` minor units of `to`. Fields: `from`,
`to` (currencies), `numerator`, `denominator` (integers in `1 ..= MAX_RATE_PART`).
JSON: `{"from": "USD", "to": "EUR", "numerator": 92, "denominator": 100}`.

## Policies (engine-wide inputs, not promotion data)

### RoundingPolicy
How `roundDiv` resolves a fractional minor unit (ADR 3). Variants:
- `HalfUp`: a remainder of exactly one half rounds away from zero.
- `HalfEven`: a remainder of exactly one half rounds to the even neighbour.
- `Down`: toward zero (equals floor for the non-negative amounts in the engine).
JSON: `{"type": "Down"}`.

### StackingPolicy
What base a percentage promotion is computed on when several promotions stack (ADR 4).
- `Sequential`: the running order subtotal, after earlier promotions.
- `Additive`: the original order subtotal.
JSON: `{"type": "Sequential"}`.

### OrderCap
The most the whole order may be discounted (ADR 4). Fields: `amount : Option Money`,
`bps : Option BasisPoints`. The cap is the minimum of the present parts, the percentage
part being `roundDiv(PricingPolicy.rounding, orderSubtotal * bps, BPS_SCALE)`. Both
absent means unconstrained.
JSON: `{"amount": null, "bps": 5000}`.

### PricingPolicy
Bundle of all policies: `rounding : RoundingPolicy`, `stacking : StackingPolicy`,
`order_cap : OrderCap`.
JSON: `{"rounding": {"type": "Down"}, "stacking": {"type": "Sequential"}, "order_cap": {...}}`.

## Cart

### CartLine
One line of the cart. Fields: `line_id : LineId`, `unit_price : Money`,
`quantity : Quantity`. Its **line subtotal** is `unit_price.amount * quantity`
(`lineSubtotal`); discounts are computed and allocated on line subtotals, never per unit
(ADR 8).
JSON: `{"line_id": "L1", "unit_price": {...}, "quantity": 2}`.

### CartSnapshot
The immutable cart as seen at pricing time. Fields: `currency : Currency`,
`lines : List CartLine`, `tags : List String` (free-form markers such as channel or
platform), `attributes : List Attribute`, `coupon_codes : List String` (codes the customer
presented; each at most once). Every line's `unit_price.currency` equals `currency`. An
empty list of lines is valid and prices to zero. The **order subtotal** (`orderSubtotal`)
is the sum of line subtotals.
JSON: `{"currency": "EUR", "lines": [...], "tags": ["#ios"], "attributes": [...], "coupon_codes": ["WELCOME"]}`.

### CustomerContext
What eligibility rules may know about the customer (ADR 7). Fields:
`customer_id : Option String`, `completed_orders : Int` (`≥ 0`; `0` means first order),
`uses : List PromotionUse` where `PromotionUse = { promotion_id, count }` (`count ≥ 0`,
each promotion at most once): how many times this customer already used each promotion.
The application layer fills `uses` from reserved and confirmed redemptions.
JSON: `{"customer_id": "c1", "completed_orders": 0, "uses": [{"promotion_id": "P1", "count": 1}]}`.

## Stacking groups

### StackingMode
- `Stackable`: every eligible promotion in the group applies, in order.
- `FirstMatch`: only the first eligible promotion of the group in the total order applies;
  the rest are skipped with `FirstMatchTaken`.
JSON: `{"type": "FirstMatch"}`.

### StackingGroup
Fields: `group_id : GroupId`, `priority : Int`, `mode : StackingMode`. Groups are an input
(`PricingInput.groups`); a promotion's `group_id` must name one of them.
JSON: `{"group_id": "shipping", "priority": 10, "mode": {"type": "FirstMatch"}}`.

### Total order
Eligible promotions are processed in the order
`(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)`
(`sortPromotions`). The order the caller lists groups or promotions in is irrelevant.

## Promotion

### Rule
A predicate over `(cart, customer, coupons, now)` and the promotion being evaluated
(OpenSpec `eligibility-rules`). Variants:
- `DateWindow { starts_at?, ends_at? }`: `starts_at ≤ now` (or absent) and `now < ends_at`
  (or absent). Half-open. `starts_at > ends_at` is a valid, always-false rule.
- `OrderTotalRange { min : Option Money, max : Option Money }`: `min ≤ orderSubtotal` and
  `orderSubtotal ≤ max` for the present bounds, after normalization to the cart currency.
- `TagsMatch { all, any, none : List String }`: every tag of `all` is in `cart.tags`; `any`
  is empty or shares a tag with `cart.tags`; no tag of `none` is in `cart.tags`.
- `AttributeIn { key, values : List String }`: some attribute has `key` and a value in `values`.
- `AttributeDateWindow { key, starts_at?, ends_at? }`: the attribute `key` exists, its value
  parses as a decimal integer `t`, and `starts_at ≤ t`, `t < ends_at` for the present bounds.
  A missing or non-integer value fails the rule.
- `MaxCompletedOrders { max : Int }`: `customer.completed_orders ≤ max` (`0` = first order only).
- `MaxUsesPerCustomer { max : Int }`: the `count` in `customer.uses` for this promotion
  (`0` when absent) is `< max`.
- `MaxUsesTotal { max : Int }`: the promotion's `uses_total` is `< max`.
- `RequiresCoupon`: some coupon in `coupons` has this `promotion_id`, its `code` is in
  `cart.coupon_codes`, its window contains `now`, its `customer_id` is absent or equals
  `customer.customer_id`, and it has remaining uses (`max_uses` absent or `uses < max_uses`).
JSON: `{"type": "DateWindow", "starts_at": 1760227200, "ends_at": null}`,
`{"type": "OrderTotalRange", "min": {"amount": 5000, "currency": "EUR"}, "max": null}`,
`{"type": "TagsMatch", "all": [], "any": ["#ios", "#android"], "none": ["#partner"]}`,
`{"type": "AttributeIn", "key": "hotel_kind", "values": ["resort"]}`,
`{"type": "AttributeDateWindow", "key": "checkin", "starts_at": 100, "ends_at": 200}`,
`{"type": "MaxCompletedOrders", "max": 0}`, `{"type": "MaxUsesPerCustomer", "max": 1}`,
`{"type": "MaxUsesTotal", "max": 100}`, `{"type": "RequiresCoupon"}`.

### Eligibility
A promotion is **eligible** for an input when every one of its rules holds (`eligible`).
No rules means always eligible. Eligibility depends on nothing but `(now, cart, customer,
coupons)` and the promotion (ADR 7). `firstFailingRule` reports the index of the first
failing rule, for `SkipReason.NotEligible`.

### Action
What an eligible promotion does. Order-level only in the first iteration (ADR 8):
- `OrderPercentage { bps : BasisPoints }`: discount = `roundDiv(effective rounding, base * bps, BPS_SCALE)`
  where `base` is chosen by `StackingPolicy` and the effective rounding is the promotion's
  `rounding` if present, else `PricingPolicy.rounding` (ADR 3).
- `OrderFixedAmount { amount : Money }`: discount = `amount.amount`.
JSON: `{"type": "OrderPercentage", "bps": 1500}`, `{"type": "OrderFixedAmount", "amount": {...}}`.

### Cap
`cap : Option Money` on a promotion: the maximum discount this promotion may grant on one
order. Absent means unlimited. Like every promotion amount it may be in another currency
than the cart's; `normalize` converts it.

### Remaining budget
`remaining_budget : Option Money` on a promotion **snapshot**: the campaign budget still
available when the snapshot was taken (ADR 7). Treated as a second cap. Consuming the
budget is the application layer's job (`Redeem`).

### Exclusive
`exclusive : Bool`. If any eligible promotion is exclusive, the first exclusive one in the
**total order** is applied alone (`selectExclusive`, ADR 4); all others are skipped with
`ExclusiveApplied`. Evaluated before group modes.

### Priority
`priority : Int`, part of the total order. Higher applies first.

### Uses total
`uses_total : Int` on a promotion **snapshot** (`≥ 0`): how many reserved or confirmed
redemptions applied it. Derived by the application layer like `remaining_budget`; read by
`MaxUsesTotal`.

### Promotion
Fields, in JSON order: `promotion_id`, `group_id`, `priority`, `exclusive`, `cap`,
`remaining_budget`, `uses_total`, `rounding : Option RoundingPolicy`, `rules : List Rule`,
`action : Action`.

## Pricing

### PricingInput
Everything `apply` may depend on (ADR 7). Fields: `now : Timestamp`,
`policy : PricingPolicy`, `customer : CustomerContext`, `cart : CartSnapshot`,
`coupons : List Coupon`, `exchange_rates : List ExchangeRate`,
`groups : List StackingGroup`, `promotions : List Promotion`.

### apply
`apply : PricingInput → Except PricingError PricingResult`. Total (ADR 6). Pipeline, with
one named function per stage in both code bases (ADR 8):

1. `validate` — first failing check in the fixed order below, or the validated input.
2. `normalize` — convert every promotion amount (`action.amount`, `cap`, `remaining_budget`,
   `OrderTotalRange.min|max`) and `policy.order_cap.amount` to the cart currency with
   `convertAmount` (OpenSpec `multi-currency`). Everything below runs on the normalized
   input, and so do the invariants.
3. `lineSubtotal`, `orderSubtotal` — integer sums (`subsOf` is the list of line subtotals).
3. `sortPromotions` — all promotions in the total order (`sortedOf`).
4. `eligible` — keep the eligible ones (`eligibleOf`); `firstFailingRule` gives the
   `NotEligible` skips (`skipsOf`).
5. `selectExclusive` — the first exclusive promotion alone, or the whole list
   (`afterExclusive`).
6. `selectFirstMatch` — per `FirstMatch` group keep the first promotion, skip the rest
   (`afterFirstMatch`, whose kept list is `toApplyOf`).
7. `applyOne` — fold over `toApplyOf` from `initStep` to `finalStep`: for each promotion
   `rawDiscount` from `Action` (with `effectiveRounding`), `clampDiscount`, `allocate` to
   lines, `subLists` the running subtotals, record `clamped_by`. The fold state is `Step`
   (`lineRunning`, `orderCapRemaining`, `applied`); `orderCapInitial` seeds the cap.
8. `compute` assembles `PricingResult`; `skipped` is `sortedOf` filtered through the
   collected skip reasons, so it is in total order.

### convertAmount
`convertAmount rates amount from to : Option Int`. Same currency: `some amount`. Otherwise
the rate with `from`/`to` (`findRate`), and `roundDiv Down (amount * numerator) denominator`;
`none` when no rate exists. Conversion rounding is always `Down`, whatever the policy.

### roundDiv
`roundDiv : RoundingPolicy → (num den : Int) → Int`, `den > 0`, `num ≥ 0` in all uses.
With `q = num / den` (floor) and `r = num % den`:
`HalfUp` gives `q + 1` if `2r ≥ den` else `q`; `HalfEven` gives `q + 1` if `2r > den`,
`q` if `2r < den`, and the even one of `q`, `q + 1` if `2r = den`; `Down` gives `q`.

### clampDiscount
`min` of the raw discount and every present bound, in this order: `cap.amount`,
`remaining_budget.amount`, the remaining order cap (`OrderCap` minus discounts applied so
far), and the running order subtotal. Result is in `0 ..= running`. Also returns the
`ClampReason`: `none` if the raw discount was not reduced, otherwise the first bound in
that order whose value equals the result.

### allocate
Splits an order-level discount `D` over line subtotals `s_i` by the largest-remainder
method with ties by line position (ADR 5). Returns one amount per line. Uses `Int` in
Lean and `i128` intermediates in Rust. Named pieces, identical in both code bases:
`baseShares` (`floor(D s_i / S)`), `remainders` (`(D s_i) mod S`), `extraUnits`
(`R = D - Σ base_i`), `positiveIndices` (candidate lines, those with a positive
remainder), `remainderLe` (the ordering: larger remainder first, then position),
`pickedLines` (the first `R` candidates in that order), `bumpAll` (one unit to each
picked line). If `S = 0` every line gets `0`.

### Running subtotal
The order subtotal minus all discounts applied so far, maintained per line and in total.
`Sequential` uses it as the base for percentages; both policies clamp against it.

### PricingError
Returned by `validate`; the **check order** is normative (ADR 6) and both implementations
report the first failure in this order. Helper names shared by both code bases:
`validCurrency`, `amountInRange`, `bpsInRange`, `ratePartInRange`, `checkMoney`,
`checkOptMoney`, `checkLines`, `firstDuplicate`, `checkDuplicates`, `checkCustomer`,
`checkRates`, `checkCoupons`, `checkOrderCap`, `checkAction`, `checkRules`,
`checkPromotions`, sequenced with `andThen`. `checkMoney` of a `Money` field is:
`InvalidCurrency` → (currency differs from the cart's and `findRate` has no rate:
`CurrencyMismatch`) → `AmountOutOfRange` on the original amount → `AmountOutOfRange` on the
converted amount.

1. `InvalidCurrency { field }` — `cart.currency` malformed.
2. For each line `i` in order: `InvalidCurrency` on `cart.lines[i].unit_price`;
   `MixedCurrencies { line_index }` if it differs from `cart.currency`;
   `AmountOutOfRange { field }` for `cart.lines[i].unit_price`;
   `InvalidQuantity { line_index }`.
3. `DuplicateLineId { line_id }` — first `line_id` seen twice, scanning in order.
4. `DuplicateCouponCode { code }` — first code seen twice in `cart.coupon_codes`.
5. `AmountOutOfRange { field: "cart.subtotal" }` — order subtotal above `MAX_MINOR_UNITS`.
6. `customer`: `InvalidCustomerContext { field: "customer.completed_orders" }` if negative;
   for each `uses[k]`: negative `count` → `InvalidCustomerContext { field: "customer.uses[<k>].count" }`;
   then a `promotion_id` seen twice → `InvalidCustomerContext { field: "customer.uses[<k>]" }`
   with `<k>` the second occurrence.
7. For each `exchange_rates[k]` in order: `InvalidCurrency` on `.from`, on `.to`;
   `InvalidExchangeRate { field }` for `.numerator`, `.denominator` outside `1 ..= MAX_RATE_PART`;
   then `DuplicateExchangeRate { from, to }` for the first pair seen twice.
8. For each `coupons[k]` in order: `InvalidCoupon { field }` for `.uses` or `.max_uses`
   negative (`coupons[<k>].uses|max_uses`); then `DuplicateCouponCode { code }` — first code
   seen twice in `coupons`.
9. `policy.order_cap`: if `amount` present, `checkMoney` on `policy.order_cap.amount`; if
   `bps` present, `InvalidBasisPoints { field: "policy.order_cap.bps" }`.
10. `DuplicateGroupId { group_id }` — first `group_id` seen twice, scanning in order.
11. For each promotion `i` in order: `InvalidBasisPoints` for `promotions[<i>].action.bps`;
    `checkMoney` for each of `action.amount`, `cap`, `remaining_budget` that is present, in
    that order; `InvalidPromotion { field: "promotions[<i>].uses_total" }` if negative; for
    each `rules[k]` in order: `OrderTotalRange` → `checkMoney` on `.min`, `.max` when present
    (`promotions[<i>].rules[<k>].min|max`); `MaxCompletedOrders`, `MaxUsesPerCustomer`,
    `MaxUsesTotal` → `InvalidRule { field: "promotions[<i>].rules[<k>].max" }` if negative;
    then `UnknownGroup { promotion_index }` if `group_id` is not in `groups`.
12. `DuplicatePromotionId { promotion_id }` — first id seen twice, scanning in order.
13. `Overflow` — Rust only, from checked arithmetic after validation; unreachable for
    validated input (ADR 3). Lean never produces it.

`field` is a path string: `cart.currency`, `cart.lines[<i>].unit_price`, `cart.subtotal`,
`customer.completed_orders`, `customer.uses[<k>]`, `customer.uses[<k>].count`,
`exchange_rates[<k>].from|to|numerator|denominator`, `coupons[<k>].uses|max_uses`,
`promotions[<i>].uses_total`, `policy.order_cap.amount`,
`policy.order_cap.bps`, `promotions[<i>].action.bps`, `promotions[<i>].action.amount`,
`promotions[<i>].cap`, `promotions[<i>].remaining_budget`, `promotions[<i>].rules[<k>].min|max`,
with `<i>`, `<k>` decimal indexes without padding.
JSON: `{"type": "AmountOutOfRange", "field": "promotions[2].cap"}`,
`{"type": "DuplicateExchangeRate", "from": "USD", "to": "EUR"}`.

### ClampReason
Why a discount is smaller than its raw value (ADR 9): `Cap`, `Budget`, `OrderCap`,
`Running`.
JSON: `{"type": "OrderCap"}`.

### SkipReason
Why a promotion was not applied (ADR 9):
- `NotEligible { rule_index }` — index of the first rule that did not hold.
- `ExclusiveApplied { winner : PromotionId }`.
- `FirstMatchTaken { winner : PromotionId }`.
JSON: `{"type": "FirstMatchTaken", "winner": "P7"}`.

### SkippedPromotion
`promotion_id`, `reason : SkipReason`.

### LineAllocation
`line_id`, `amount : Money` — what one applied promotion took from one line.

### AppliedPromotion
One step of the fold, in application order: `promotion_id`, `discount : Money`,
`clamped_by : Option ClampReason`, `allocation : List LineAllocation` (one entry per
cart line, in cart order).

### LineResult
Per line, in cart order: `line_id`, `subtotal : Money`, `discount : Money`, `total : Money`
with `total = subtotal - discount`.

### PricingResult
`currency`, `original_subtotal : Money`, `total_discount : Money`, `final_total : Money`,
`lines : List LineResult`, `applied : List AppliedPromotion`,
`skipped : List SkippedPromotion` (in total order).
`final_total = original_subtotal - total_discount`. Every input promotion is in exactly
one of `applied` and `skipped`.

### Result envelope (CLI contract)
The Lean reference executable is line-oriented: one `PricingInput` JSON document per
input line, one envelope per output line, `{"ok": <PricingResult>}` or
`{"error": <PricingError>}`. A line that does not decode yields
`{"decode_error": "<message>"}`; the DRT treats that as a harness bug, never as a result.

## Application layer (not verified, named here for consistency)

- `Evaluate`: use case that loads promotion snapshots and groups through
  `PromotionRepository`, reads `Clock`, builds a `PricingInput` and calls `apply`.
  Pure with respect to the domain: a dry run.
- `Redeem`: use case that records an `Evaluate` result for an order and consumes budgets
  and usage counters. Iteration 2 (ADR 7, ADR 4).
- `SavedCoupon { customer_id, code }`: a public code a customer keeps; stored in the
  catalogue, never read by pricing (OpenSpec change `customer-coupons`).
- `CustomerCoupon { coupon, status, order_id?, promotion? }`: read model returned by
  `ListCustomerCoupons`; `promotion` is `{ promotion_id, action }` of the coupon's promotion.
- `CouponStatus`: `applied` (the customer used the coupon; `order_id` names the earliest such
  redemption) | `expired` (`ends_at ≤ now`, or `uses ≥ max_uses`, or the promotion is
  archived or missing) | `scheduled` (`starts_at > now`) | `active`. Cart-free: whether the
  coupon applies to a cart is `Evaluate`'s answer.
- `ListCustomerCoupons`: use case that merges the customer's bound, used and saved coupons
  and computes each `CouponStatus` at `Clock.now`.

## Telemetry vocabulary (not verified; `src/infrastructure/`)

Names the service emits over OpenTelemetry. They reuse the domain vocabulary above and are
the contract the Grafana dashboards depend on (OpenSpec change `otel-grafana-observability`).
`src/infrastructure/src/telemetry.rs` keeps the same list as constants; a test checks the two agree.

- Resource: `service.name` = `discount`, `service.version`, `deployment.environment`.
- Span `discount.evaluate` (child of the HTTP server span) with attributes
  `discount.currency`, `discount.lines`, `discount.promotions`, `discount.applied`,
  `discount.skipped`, `discount.original_subtotal`, `discount.total_discount` and, on error,
  `discount.error_type` (a `PricingError` variant name).
- Metrics (OpenTelemetry names; Prometheus sees the translated `discount_…` forms):
  `discount.evaluations` {`outcome` = `ok` | `error`, `error_type`},
  `discount.evaluate.duration` seconds {`outcome`},
  `discount.applied` {`promotion_id`, `clamped_by` = `none` | `Cap` | `Budget` | `OrderCap` | `Running`},
  `discount.applied.amount` minor units {`promotion_id`, `currency`},
  `discount.skipped` {`promotion_id`, `reason` = `NotEligible` | `ExclusiveApplied` | `FirstMatchTaken`},
  `discount.amount` minor units {`currency`},
  `discount.share.bps` basis points of the original subtotal, buckets
  `0, 100, 250, 500, 1000, 1500, 2000, 3000, 5000, 7500, 10000`.
  `discount.redemptions` {`action` = `reserve` | `confirm` | `cancel`, `outcome` = `ok` | `idempotent` | `error`},
  `discount.budget.remaining` gauge, minor units {`promotion_id`, `currency`}.
- One log record per evaluation, `INFO` on success and `WARN` on a pricing error, carrying
  `outcome`, `total_discount` and the trace and span ids of `discount.evaluate`.
