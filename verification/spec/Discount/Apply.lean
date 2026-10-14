import Discount.Rounding
import Discount.Validate
import Discount.Eligibility
import Discount.Stacking
import Discount.Allocate

/-!
# apply (ADR 4, ADR 6, ADR 9)

`validate → sortPromotions → eligible → selectExclusive → selectFirstMatch → fold applyOne`.
-/
namespace Discount

/-- Intermediate state of the fold. -/
structure Step where
  lineRunning : List Int
  orderCapRemaining : Option Int
  applied : List AppliedPromotion
  deriving Repr

def effectiveRounding (policy : PricingPolicy) (p : Promotion) : RoundingPolicy :=
  match p.rounding with
  | some r => r
  | none => policy.rounding

def rawDiscount (policy : PricingPolicy) (original running : Int) (p : Promotion) : Int :=
  let base := match policy.stacking with
    | .Sequential => running
    | .Additive => original
  match p.action with
  | .OrderPercentage bps => roundDiv (effectiveRounding policy p) (base * bps) BPS_SCALE
  | .OrderFixedAmount m => m.amount

/-- `min(raw, cap, remaining_budget, remaining_order_cap, running)` and the first bound in
that order whose value equals the result (`none` if the raw discount was not reduced). -/
def clampDiscount (raw : Int) (cap budget orderCapRem : Option Int) (running : Int) :
    Int × Option ClampReason :=
  let d1 := match cap with | some c => min raw c | none => raw
  let d2 := match budget with | some b => min d1 b | none => d1
  let d3 := match orderCapRem with | some o => min d2 o | none => d2
  let d := min d3 running
  let reason : Option ClampReason :=
    if d = raw then none
    else if cap = some d then some .Cap
    else if budget = some d then some .Budget
    else if orderCapRem = some d then some .OrderCap
    else some .Running
  (d, reason)

def orderCapInitial (policy : PricingPolicy) (S : Int) : Option Int :=
  match policy.order_cap.amount, policy.order_cap.bps with
  | none, none => none
  | some m, none => some m.amount
  | none, some b => some (roundDiv policy.rounding (S * b) BPS_SCALE)
  | some m, some b => some (min m.amount (roundDiv policy.rounding (S * b) BPS_SCALE))

/-- Pointwise subtraction, `a - b`, over lists of equal length. -/
def subLists : List Int → List Int → List Int
  | a :: as, b :: bs => (a - b) :: subLists as bs
  | _, _ => []

def applyOne (i : PricingInput) (original : Int) (st : Step) (p : Promotion) : Step :=
  let running := st.lineRunning.sum
  let raw := rawDiscount i.policy original running p
  let dr := clampDiscount raw (p.cap.map (·.amount)) (p.remaining_budget.map (·.amount))
    st.orderCapRemaining running
  let d := dr.1
  let alloc := allocate d st.lineRunning
  let cur := i.cart.currency
  { lineRunning := subLists st.lineRunning alloc
    orderCapRemaining := st.orderCapRemaining.map (fun o => o - d)
    applied := st.applied ++ [{
      promotion_id := p.promotion_id
      discount := ⟨d, cur⟩
      clamped_by := dr.2
      allocation := List.zipWith (fun (l : CartLine) (a : Int) =>
        ({ line_id := l.line_id, amount := ⟨a, cur⟩ } : LineAllocation)) i.cart.lines alloc }] }

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

/-! ## normalize (OpenSpec `multi-currency`) -/

/-- `m` in the cart currency; unchanged when no rate exists (validation guarantees one). -/
def normalizeMoney (rates : List ExchangeRate) (cur : Currency) (m : Money) : Money :=
  match convertAmount rates m.amount m.currency cur with
  | some a => { amount := a, currency := cur }
  | none => m

def normalizeRule (rates : List ExchangeRate) (cur : Currency) : Rule → Rule
  | .OrderTotalRange mn mx => .OrderTotalRange (mn.map (normalizeMoney rates cur)) (mx.map (normalizeMoney rates cur))
  | r => r

def normalizeAction (rates : List ExchangeRate) (cur : Currency) : Action → Action
  | .OrderFixedAmount m => .OrderFixedAmount (normalizeMoney rates cur m)
  | a => a

def normalizePromotion (rates : List ExchangeRate) (cur : Currency) (p : Promotion) : Promotion :=
  { p with
    cap := p.cap.map (normalizeMoney rates cur)
    remaining_budget := p.remaining_budget.map (normalizeMoney rates cur)
    rules := p.rules.map (normalizeRule rates cur)
    action := normalizeAction rates cur p.action }

/-- Every promotion amount and the order cap in the cart currency. Everything else is
unchanged. -/
def normalize (i : PricingInput) : PricingInput :=
  let cur := i.cart.currency
  let rates := i.exchange_rates
  { i with
    policy := { i.policy with
      order_cap := { i.policy.order_cap with amount := i.policy.order_cap.amount.map (normalizeMoney rates cur) } }
    promotions := i.promotions.map (normalizePromotion rates cur) }

def apply (i : PricingInput) : Except PricingError PricingResult :=
  match validate i with
  | .error e => .error e
  | .ok () => .ok (compute (normalize i))

end Discount
