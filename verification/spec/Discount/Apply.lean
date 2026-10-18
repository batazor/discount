import Discount.Apply.Step
import Discount.Apply.Normalize
import Discount.Eligibility
import Discount.Stacking

/-!
# apply (ADR 4, ADR 6, ADR 9)

`validate → normalize → sortPromotions → eligible → selectExclusive → selectFirstMatch → fold applyOne`.
-/
namespace Discount

def skipsOf (i : PricingInput) (sorted : List Promotion) : List (PromotionId × SkipReason) :=
  sorted.filterMap (fun p =>
    match firstFailingRule p.rules p i 0 with
    | some k => some (p.promotion_id, SkipReason.NotEligible k)
    | none => none)

/-- Line subtotals in cart order. -/
def subsOf (i : PricingInput) : List Int := i.cart.lines.map lineSubtotal

/-- All promotions in the total order. -/
def sortedOf (i : PricingInput) : List Promotion := sortPromotions i.groups i.promotions

/-- Eligible promotions, in the total order. -/
def eligibleOf (i : PricingInput) : List Promotion := (sortedOf i).filter (fun p => eligible p i)

/-- After exclusivity. -/
def afterExclusive (i : PricingInput) : List Promotion × List (PromotionId × SkipReason) :=
  selectExclusive (eligibleOf i)

/-- After first-match groups: the promotions that are actually applied, in order. -/
def afterFirstMatch (i : PricingInput) : List Promotion × List (PromotionId × SkipReason) :=
  selectFirstMatch i.groups (afterExclusive i).1 []

def toApplyOf (i : PricingInput) : List Promotion := (afterFirstMatch i).1

def initStep (i : PricingInput) : Step :=
  { lineRunning := subsOf i, orderCapRemaining := orderCapInitial i.policy (subsOf i).sum, applied := [] }

def finalStep (i : PricingInput) : Step :=
  (toApplyOf i).foldl (applyOne i (subsOf i).sum) (initStep i)

/-- The pure computation after validation. -/
def compute (i : PricingInput) : PricingResult :=
  let cur := i.cart.currency
  let subs := subsOf i
  let S := subs.sum
  let skips := skipsOf i (sortedOf i) ++ (afterExclusive i).2 ++ (afterFirstMatch i).2
  let final := finalStep i
  let D := S - final.lineRunning.sum
  { currency := cur
    original_subtotal := ⟨S, cur⟩
    total_discount := ⟨D, cur⟩
    final_total := ⟨S - D, cur⟩
    lines := List.zipWith (fun (l : CartLine) (sl : Int × Int) =>
      ({ line_id := l.line_id, subtotal := ⟨sl.1, cur⟩, discount := ⟨sl.1 - sl.2, cur⟩,
         total := ⟨sl.2, cur⟩ } : LineResult)) i.cart.lines (subs.zip final.lineRunning)
    applied := final.applied
    skipped := (sortedOf i).filterMap (fun p =>
      match skips.lookup p.promotion_id with
      | some r => some { promotion_id := p.promotion_id, reason := r }
      | none => none) }

def apply (i : PricingInput) : Except PricingError PricingResult :=
  match validate i with
  | .error e => .error e
  | .ok () => .ok (compute (normalize i))

end Discount
