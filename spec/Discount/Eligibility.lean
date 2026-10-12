import Discount.Types

/-!
# eligible (ADR 7)

A promotion is eligible when every rule holds. `firstFailingRule` reports the index of the
first rule that does not hold, for `SkipReason.NotEligible`.
-/
namespace Discount

def ruleHolds (r : Rule) (i : PricingInput) : Bool :=
  match r with
  | .DateWindow starts_at ends_at =>
      (match starts_at with | none => true | some s => decide (s ≤ i.now)) &&
      (match ends_at with | none => true | some e => decide (i.now < e))

def firstFailingRule : List Rule → PricingInput → Nat → Option Nat
  | [], _, _ => none
  | r :: rs, i, k => if ruleHolds r i then firstFailingRule rs i (k + 1) else some k

def eligible (p : Promotion) (i : PricingInput) : Bool :=
  (firstFailingRule p.rules i 0).isNone

end Discount
