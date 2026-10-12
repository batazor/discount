# 7. Purity: clock, budget and customer context enter only as inputs

Date: 2026-10-12

## Status

Accepted

## Context

The pricing core has no IO, no async, no clock and no randomness (project decision). Some
promotion semantics look stateful: "valid until", "first order only", a campaign budget
that depletes across orders (Medusa). The Lean model cannot read a database.

Touches invariant 7 (eligibility depends only on its inputs).

Options considered:

1. Pass a `Clock`/`BudgetLedger` port into the core. Rejected: ports are an
   application-layer concept; a trait object in the core breaks the Aeneas subset and
   makes the Lean mirror awkward.
2. Snapshot inputs. Chosen.

## Decision

Everything that could vary between two calls is a field of `PricingInput`:

- `now : Timestamp`: Unix seconds, `i64`/`Int`. Time zones do not exist in the core; a
  `DateWindow` rule compares instants. Callers convert.
- `customer : CustomerContext`: whatever eligibility rules need about the customer. In
  the first iteration it is empty; the type exists so the shape of the input is stable.
- `remaining_budget : Option Money` on each `Promotion` snapshot: the budget still
  available for this promotion when the snapshot was taken. The core treats it as one
  more cap (ADR 4). Reserving or decrementing the budget is an application-layer concern
  (`app/`), done after pricing, with its own tests; the core never sees the ledger.
- The promotions themselves are passed in as a list, already loaded by the application
  layer via `PromotionRepository`.

The core exports a single entry point, `apply`, and no function with observable effects.
Invariant 7 is therefore true by construction: the function has no other inputs. The Lean
statement is a trivial theorem kept only as documentation.

## Consequences

- Budget consumption is eventually consistent: two concurrent orders may both see the
  same `remaining_budget`. Enforcement belongs to the application layer and gets its own
  ADR when budgets are implemented beyond the cap.
- The `PricingInput` JSON document is self-contained and replayable: a production pricing
  result can be reproduced bit-for-bit from its stored input, in either implementation.
  This is also what makes DRT trivially possible.
