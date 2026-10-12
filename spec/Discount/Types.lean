/-!
# Domain types

Names follow `docs/ubiquitous-language.md` exactly (ADR 8). Constructor names are the
JSON variant names. Field names are the JSON keys.
-/
namespace Discount

abbrev Currency := String
abbrev LineId := String
abbrev GroupId := String
abbrev PromotionId := String
abbrev BasisPoints := Int
abbrev Timestamp := Int
abbrev Quantity := Int

/-- Upper bound for every amount and for the order subtotal (ADR 3). -/
def MAX_MINOR_UNITS : Int := 100000000000000
/-- Upper bound for `quantity`. -/
def MAX_QUANTITY : Int := 1000000000
/-- Denominator of `BasisPoints`. -/
def BPS_SCALE : Int := 10000

structure Money where
  amount : Int
  currency : Currency
  deriving Repr, DecidableEq, Inhabited

inductive RoundingPolicy where
  | HalfUp
  | HalfEven
  | Down
  deriving Repr, DecidableEq, Inhabited

inductive StackingPolicy where
  | Sequential
  | Additive
  deriving Repr, DecidableEq, Inhabited

structure OrderCap where
  amount : Option Money
  bps : Option BasisPoints
  deriving Repr, DecidableEq, Inhabited

structure PricingPolicy where
  rounding : RoundingPolicy
  stacking : StackingPolicy
  order_cap : OrderCap
  deriving Repr, DecidableEq, Inhabited

structure CartLine where
  line_id : LineId
  unit_price : Money
  quantity : Quantity
  deriving Repr, DecidableEq, Inhabited

structure CartSnapshot where
  currency : Currency
  lines : List CartLine
  deriving Repr, DecidableEq, Inhabited

/-- Empty in the first iteration; exists so the input shape is stable (ADR 7). -/
structure CustomerContext where
  deriving Repr, DecidableEq, Inhabited

inductive StackingMode where
  | Stackable
  | FirstMatch
  deriving Repr, DecidableEq, Inhabited

structure StackingGroup where
  group_id : GroupId
  priority : Int
  mode : StackingMode
  deriving Repr, DecidableEq, Inhabited

inductive Rule where
  /-- Holds when `starts_at ≤ now` (or absent) and `now < ends_at` (or absent). -/
  | DateWindow (starts_at : Option Timestamp) (ends_at : Option Timestamp)
  deriving Repr, DecidableEq, Inhabited

inductive Action where
  | OrderPercentage (bps : BasisPoints)
  | OrderFixedAmount (amount : Money)
  deriving Repr, DecidableEq, Inhabited

structure Promotion where
  promotion_id : PromotionId
  group_id : GroupId
  priority : Int
  exclusive : Bool
  cap : Option Money
  remaining_budget : Option Money
  rounding : Option RoundingPolicy
  rules : List Rule
  action : Action
  deriving Repr, DecidableEq, Inhabited

structure PricingInput where
  now : Timestamp
  policy : PricingPolicy
  customer : CustomerContext
  cart : CartSnapshot
  groups : List StackingGroup
  promotions : List Promotion
  deriving Repr, DecidableEq, Inhabited

inductive PricingError where
  | InvalidCurrency (field : String)
  | MixedCurrencies (line_index : Nat)
  | CurrencyMismatch (field : String)
  | AmountOutOfRange (field : String)
  | InvalidBasisPoints (field : String)
  | InvalidQuantity (line_index : Nat)
  | DuplicateLineId (line_id : LineId)
  | DuplicateGroupId (group_id : GroupId)
  | DuplicatePromotionId (promotion_id : PromotionId)
  | UnknownGroup (promotion_index : Nat)
  /-- Rust only; the Lean model never produces it (ADR 3). -/
  | Overflow
  deriving Repr, DecidableEq, Inhabited

inductive ClampReason where
  | Cap
  | Budget
  | OrderCap
  | Running
  deriving Repr, DecidableEq, Inhabited

inductive SkipReason where
  | NotEligible (rule_index : Nat)
  | ExclusiveApplied (winner : PromotionId)
  | FirstMatchTaken (winner : PromotionId)
  deriving Repr, DecidableEq, Inhabited

structure SkippedPromotion where
  promotion_id : PromotionId
  reason : SkipReason
  deriving Repr, DecidableEq, Inhabited

structure LineAllocation where
  line_id : LineId
  amount : Money
  deriving Repr, DecidableEq, Inhabited

structure AppliedPromotion where
  promotion_id : PromotionId
  discount : Money
  clamped_by : Option ClampReason
  allocation : List LineAllocation
  deriving Repr, DecidableEq, Inhabited

structure LineResult where
  line_id : LineId
  subtotal : Money
  discount : Money
  total : Money
  deriving Repr, DecidableEq, Inhabited

structure PricingResult where
  currency : Currency
  original_subtotal : Money
  total_discount : Money
  final_total : Money
  lines : List LineResult
  applied : List AppliedPromotion
  skipped : List SkippedPromotion
  deriving Repr, DecidableEq, Inhabited

/-- `unit_price.amount * quantity` (ADR 8). -/
def lineSubtotal (l : CartLine) : Int := l.unit_price.amount * l.quantity

/-- Sum of line subtotals. -/
def orderSubtotal (c : CartSnapshot) : Int := (c.lines.map lineSubtotal).sum

end Discount
