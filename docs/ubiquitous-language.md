# Ubiquitous language

The single list of names used in `spec/` (Lean 4), `core/` (Rust), `docs/invariants.md`
and in conversation. A name here is spelled identically in both code bases (modulo
language casing: `CartSnapshot` in both, `lineSubtotal` in Lean / `line_subtotal` in Rust).
Adding a concept means adding it here first (ADR 8).

JSON encodings are normative: they are the DRT contract between the Lean reference and
the Rust core (ADR 2). Rules shared by every type:

- Objects carry exactly the listed keys, in the listed order on output. Extra keys on
  input are an error at decode time, not a `PricingError`.
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

### LineId, PromotionId
Opaque identifiers. `PromotionId` is totally ordered by bytewise string comparison;
that order is part of the stacking semantics (ADR 4). Each is unique within its list.
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
Default when a caller does not specify: `HalfUp`.
JSON: `{"type": "HalfUp"}`.

### StackingPolicy
What base a percentage promotion is computed on when several promotions stack (ADR 4).
- `Sequential` (default): the running order subtotal, after earlier promotions.
- `Additive`: the original order subtotal.
JSON: `{"type": "Sequential"}`.

### PricingPolicy
Bundle of all policies: `rounding : RoundingPolicy`, `stacking : StackingPolicy`.
JSON: `{"rounding": {"type": "HalfUp"}, "stacking": {"type": "Sequential"}}`.

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

### Action
What an eligible promotion does. Order-level only in the first iteration (ADR 8):
- `OrderPercentage { bps : BasisPoints }`: discount = `roundDiv(rounding, base * bps, BPS_SCALE)`
  where `base` is chosen by `StackingPolicy`.
- `OrderFixedAmount { amount : Money }`: discount = `amount.amount`.
JSON: `{"type": "OrderPercentage", "bps": 1500}`, `{"type": "OrderFixedAmount", "amount": {...}}`.

### Cap
`cap : Option Money` on a promotion: the maximum discount this promotion may grant on one
order. Absent means unlimited.

### Remaining budget
`remaining_budget : Option Money` on a promotion **snapshot**: the campaign budget still
available when the snapshot was taken (ADR 7). Treated as a second cap. Consuming the
budget is the application layer's job.

### Exclusive
`exclusive : Bool`. If any eligible promotion is exclusive, the first exclusive one in the
**total order** is applied alone (`selectExclusive`, ADR 4).

### Priority and total order
`priority : Int`. Eligible promotions are processed in the **total order**
`(priority DESC, promotion_id ASC)` (`sortPromotions`). The order the caller lists
promotions in is irrelevant.

### Promotion
Fields, in JSON order: `promotion_id`, `priority`, `exclusive`, `cap`, `remaining_budget`,
`rules : List Rule`, `action : Action`.

## Pricing

### PricingInput
Everything `apply` may depend on (ADR 7). Fields: `now : Timestamp`,
`policy : PricingPolicy`, `customer : CustomerContext`, `cart : CartSnapshot`,
`promotions : List Promotion`.

### apply
`apply : PricingInput → Except PricingError PricingResult`. Total (ADR 6). Pipeline, with
one named function per stage in both code bases (ADR 8):

1. `validate` — first failing check in the fixed order below, or the validated input.
2. `lineSubtotal`, `orderSubtotal` — integer sums.
3. `eligible` — filter promotions by their rules.
4. `sortPromotions` — total order.
5. `selectExclusive` — the first exclusive promotion alone, or the whole list.
6. `applyOne` — for each remaining promotion in order: raw discount from `Action`,
   `clampDiscount` to `min(raw, cap, remaining_budget, running)`, `allocate` to lines,
   decrease the running subtotals. `roundDiv` is used inside for percentages.
7. Assemble `PricingResult`.

### roundDiv
`roundDiv : RoundingPolicy → (num den : Int) → Int`, `den > 0`, `num ≥ 0` in all uses.
With `q = num / den` (floor) and `r = num % den`:
`HalfUp` gives `q + 1` if `2r ≥ den` else `q`; `HalfEven` gives `q + 1` if `2r > den`,
`q` if `2r < den`, and the even one of `q`, `q + 1` if `2r = den`; `Down` gives `q`.

### clampDiscount
`min` of the raw discount and every present cap: `cap.amount`, `remaining_budget.amount`,
and the running order subtotal. Result is in `0 ..= running`.

### allocate
Splits an order-level discount `D` over line subtotals `s_i` by the largest-remainder
method with ties by line position (ADR 5). Returns one amount per line. Uses `Int` in
Lean and `i128` intermediates in Rust.

### Running subtotal
The order subtotal minus all discounts applied so far, maintained per line and in total.
`Sequential` uses it as the base for percentages; both policies clamp against it.

### PricingError
Returned by `validate`; the **check order** is normative (ADR 6) and both implementations
report the first failure in this order:

1. `InvalidCurrency { field }` — `cart.currency` malformed.
2. For each line `i` in order: `InvalidCurrency` on `cart.lines[i].unit_price`;
   `MixedCurrencies { line_index }` if it differs from `cart.currency`;
   `AmountOutOfRange { field }` for `cart.lines[i].unit_price`;
   `InvalidQuantity { line_index }`.
3. `DuplicateLineId { line_id }` — first `line_id` seen twice, scanning in order.
4. `AmountOutOfRange { field: "cart.subtotal" }` — order subtotal above `MAX_MINOR_UNITS`.
5. For each promotion `i` in order: `InvalidBasisPoints { promotion_index }`;
   for each of `action.amount`, `cap`, `remaining_budget` that is present, in that order:
   `InvalidCurrency`, `CurrencyMismatch { promotion_index, field }`, `AmountOutOfRange`.
6. `DuplicatePromotionId { promotion_id }` — first id seen twice, scanning in order.
7. `Overflow` — Rust only, from checked arithmetic after validation; unreachable for
   validated input (ADR 3). Lean never produces it.

`field` is a path string: `cart.currency`, `cart.lines[<i>].unit_price`, `cart.subtotal`,
`promotions[<i>].action.amount`, `promotions[<i>].cap`, `promotions[<i>].remaining_budget`,
with `<i>` a decimal index without padding.
JSON: `{"type": "AmountOutOfRange", "field": "promotions[2].cap"}`.

### LineAllocation
`line_id`, `amount : Money` — what one applied promotion took from one line.

### AppliedPromotion
One step of the fold, in application order: `promotion_id`, `discount : Money`,
`allocation : List LineAllocation` (one entry per cart line, in cart order).

### LineResult
Per line, in cart order: `line_id`, `subtotal : Money`, `discount : Money`, `total : Money`
with `total = subtotal - discount`.

### PricingResult
`currency`, `original_subtotal : Money`, `total_discount : Money`, `final_total : Money`,
`lines : List LineResult`, `applied : List AppliedPromotion`.
`final_total = original_subtotal - total_discount`.

### Result envelope (CLI contract)
The Lean executable and the Rust harness exchange
`{"ok": <PricingResult>}` or `{"error": <PricingError>}`.
