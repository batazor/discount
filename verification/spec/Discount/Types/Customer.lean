import Discount.Types.Cart

/-!
# Customer context and coupons (ADR 7, OpenSpec `coupons`)

What eligibility may know about the customer, and the codes that turn promotions on.
-/
namespace Discount

/-- How many times the customer already used a promotion. -/
structure PromotionUse where
  promotion_id : PromotionId
  count : Int
  deriving Repr, DecidableEq, Inhabited

/-- What eligibility rules may know about the customer (ADR 7). -/
structure CustomerContext where
  customer_id : Option String
  completed_orders : Int
  uses : List PromotionUse
  /-- Caller-supplied facts about the customer (`tier = "gold"`, `completed_app_orders = "0"`). -/
  attributes : List Attribute
  /-- Promotions whose allowlist names this customer; filled by the service like `uses`. -/
  allowlisted : List PromotionId
  deriving Repr, DecidableEq, Inhabited

/-- A code that turns a promotion on. -/
structure Coupon where
  code : String
  promotion_id : PromotionId
  starts_at : Option Timestamp
  ends_at : Option Timestamp
  customer_id : Option String
  /-- Absent = unlimited, `1` = single use. -/
  max_uses : Option Int
  /-- Snapshot counter: reserved or confirmed redemptions that used the code. -/
  uses : Int
  deriving Repr, DecidableEq, Inhabited

end Discount
