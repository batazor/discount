# Ubiquitous language

The single list of names used in `spec/` (Lean 4), `core/` (Rust), `docs/invariants.md`
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
`lines : List CartLine`. Every line's `unit_price.currency` equals `currency`. An empty
list is valid and prices to zero. The **order subtotal** (`orderSubtotal`) is the sum of
line subtotals.
JSON: `{"currency": "EUR", "lines": [...]}`.

### CustomerContext
What eligibility rules may know about the customer. Empty in the first iteration; the
type exists so the input shape is stable (ADR 7).
JSON: `{}`.

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
A predicate over `(CartSnapshot, CustomerContext, now)`. Variants in the first iteration:
- `DateWindow { starts_at : Option Timestamp, ends_at : Option Timestamp }`: holds when
  `starts_at ≤ now` (or `starts_at` absent) and `now < ends_at` (or `ends_at` absent).
  Half-open. `starts_at > ends_at` is a valid, always-false rule.
JSON: `{"type": "DateWindow", "starts_at": 1760227200, "ends_at": null}`.

### Eligibility
A promotion is **eligible** for an input when every one of its rules holds (`eligible`).
No rules means always eligible. Eligibility depends on nothing but the input (ADR 7).
`eligible` also reports the index of the first failing rule, for `SkipReason.NotEligible`.

### Action
What an eligible promotion does. Order-level only in the first iteration (ADR 8):
- `OrderPercentage { bps : BasisPoints }`: discount = `roundDiv(effective rounding, base * bps, BPS_SCALE)`
  where `base` is chosen by `StackingPolicy` and the effective rounding is the promotion's
  `rounding` if present, else `PricingPolicy.rounding` (ADR 3).
- `OrderFixedAmount { amount : Money }`: discount = `amount.amount`.
JSON: `{"type": "OrderPercentage", "bps": 1500}`, `{"type": "OrderFixedAmount", "amount": {...}}`.

### Cap
`cap : Option Money` on a promotion: the maximum discount this promotion may grant on one
order. Absent means unlimited.

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

### Promotion
Fields, in JSON order: `promotion_id`, `group_id`, `priority`, `exclusive`, `cap`,
`remaining_budget`, `rounding : Option RoundingPolicy`, `rules : List Rule`, `action : Action`.

## Pricing

### PricingInput
Everything `apply` may depend on (ADR 7). Fields: `now : Timestamp`,
`policy : PricingPolicy`, `customer : CustomerContext`, `cart : CartSnapshot`,
`groups : List StackingGroup`, `promotions : List Promotion`.

### apply
`apply : PricingInput → Except PricingError PricingResult`. Total (ADR 6). Pipeline, with
one named function per stage in both code bases (ADR 8):

1. `validate` — first failing check in the fixed order below, or the validated input.
2. `lineSubtotal`, `orderSubtotal` — integer sums (`subsOf` is the list of line subtotals).
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
`validCurrency`, `amountInRange`, `bpsInRange`, `checkMoney`, `checkOptMoney`,
`checkLines`, `firstDuplicate`, `checkDuplicates`, `checkOrderCap`, `checkAction`,
`checkPromotions`, sequenced with `andThen`.

1. `InvalidCurrency { field }` — `cart.currency` malformed.
2. For each line `i` in order: `InvalidCurrency` on `cart.lines[i].unit_price`;
   `MixedCurrencies { line_index }` if it differs from `cart.currency`;
   `AmountOutOfRange { field }` for `cart.lines[i].unit_price`;
   `InvalidQuantity { line_index }`.
3. `DuplicateLineId { line_id }` — first `line_id` seen twice, scanning in order.
4. `AmountOutOfRange { field: "cart.subtotal" }` — order subtotal above `MAX_MINOR_UNITS`.
5. `policy.order_cap`: if `amount` present, `InvalidCurrency`, `CurrencyMismatch { field }`,
   `AmountOutOfRange` for `policy.order_cap.amount`; if `bps` present,
   `InvalidBasisPoints { field: "policy.order_cap.bps" }`.
6. `DuplicateGroupId { group_id }` — first `group_id` seen twice, scanning in order.
7. For each promotion `i` in order: `InvalidBasisPoints` for `promotions[<i>].action.bps`;
   for each of `action.amount`, `cap`, `remaining_budget` that is present, in that order:
   `InvalidCurrency`, `CurrencyMismatch { field }`, `AmountOutOfRange`;
   then `UnknownGroup { promotion_index }` if `group_id` is not in `groups`.
8. `DuplicatePromotionId { promotion_id }` — first id seen twice, scanning in order.
9. `Overflow` — Rust only, from checked arithmetic after validation; unreachable for
   validated input (ADR 3). Lean never produces it.

`field` is a path string: `cart.currency`, `cart.lines[<i>].unit_price`, `cart.subtotal`,
`policy.order_cap.amount`, `policy.order_cap.bps`, `promotions[<i>].action.bps`,
`promotions[<i>].action.amount`, `promotions[<i>].cap`, `promotions[<i>].remaining_budget`,
with `<i>` a decimal index without padding.
JSON: `{"type": "AmountOutOfRange", "field": "promotions[2].cap"}`.

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

## Telemetry vocabulary (not verified; `adapters/`)

Names the service emits over OpenTelemetry. They reuse the domain vocabulary above and are
the contract the Grafana dashboards depend on (OpenSpec change `otel-grafana-observability`).
`adapters/src/telemetry.rs` keeps the same list as constants; a test checks the two agree.

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
- One log record per evaluation, `INFO` on success and `WARN` on a pricing error, carrying
  `outcome`, `total_discount` and the trace and span ids of `discount.evaluate`.
