import Discount.Types.Money

/-!
# Promotions, groups, rules and exchange rates (ADR 4, OpenSpec `eligibility-rules`,
`multi-currency`)
-/
namespace Discount

/-- One minor unit of `from` is `numerator / denominator` minor units of `to`. -/
structure ExchangeRate where
  «from» : Currency
  to : Currency
  numerator : Int
  denominator : Int
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
  /-- `min ≤ orderSubtotal ≤ max` for the present bounds (cart currency after normalization). -/
  | OrderTotalRange (min : Option Money) (max : Option Money)
  /-- All of `all`, at least one of `any` (if non-empty), none of `none` in `cart.tags`. -/
  | TagsMatch (all : List String) (any : List String) (none : List String)
  /-- Some attribute `key` has a value in `values`. -/
  | AttributeIn (key : String) (values : List String)
  /-- Attribute `key` parses as an integer `t` with `starts_at ≤ t < ends_at`. -/
  | AttributeDateWindow (key : String) (starts_at : Option Timestamp) (ends_at : Option Timestamp)
  /-- `customer.completed_orders ≤ max`. -/
  | MaxCompletedOrders (max : Int)
  /-- The customer's use count of this promotion is `< max`. -/
  | MaxUsesPerCustomer (max : Int)
  /-- The promotion's `uses_total` is `< max`. -/
  | MaxUsesTotal (max : Int)
  /-- A presented, valid coupon of this promotion exists. -/
  | RequiresCoupon
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
  /-- Snapshot counter: reserved or confirmed redemptions that applied the promotion. -/
  uses_total : Int
  rounding : Option RoundingPolicy
  rules : List Rule
  action : Action
  deriving Repr, DecidableEq, Inhabited

end Discount
