import Discount.Types.Cart
import Discount.Types.Customer
import Discount.Types.Promotion

/-!
# Pricing policy and the engine input (ADR 3, ADR 4)

`PricingInput` is the whole input of `apply`: nothing else is read (ADR 7).
-/
namespace Discount

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

structure PricingInput where
  now : Timestamp
  policy : PricingPolicy
  customer : CustomerContext
  cart : CartSnapshot
  coupons : List Coupon
  exchange_rates : List ExchangeRate
  groups : List StackingGroup
  promotions : List Promotion
  deriving Repr, DecidableEq, Inhabited

end Discount
