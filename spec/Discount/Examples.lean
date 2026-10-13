import Discount.Eligibility

/-!
Executable checks of the rule scenarios in OpenSpec `eligibility-rules` and `coupons`.
`#guard` fails the build when a scenario does not hold.
-/
namespace Discount.Examples

def eur (a : Int) : Money := { amount := a, currency := "EUR" }

def cart : CartSnapshot :=
  { currency := "EUR"
    lines := [{ line_id := "L1", unit_price := eur 1999, quantity := 2 },
              { line_id := "L2", unit_price := eur 501, quantity := 1 }]
    tags := ["#ios", "#promo"]
    attributes := [{ key := "checkin", value := "150" }, { key := "hotel_kind", value := "resort" }]
    coupon_codes := ["WELCOME"] }

def customer : CustomerContext :=
  { customer_id := some "c1", completed_orders := 1, uses := [{ promotion_id := "P1", count := 2 }] }

def promo (rules : List Rule) : Promotion :=
  { promotion_id := "P1", group_id := "g", priority := 0, exclusive := false, cap := none,
    remaining_budget := none, rounding := none, rules := rules, action := .OrderPercentage 1000 }

def input : PricingInput :=
  { now := 1000
    policy := { rounding := .Down, stacking := .Sequential, order_cap := { amount := none, bps := none } }
    customer := customer
    cart := cart
    coupons := [{ code := "WELCOME", promotion_id := "P1", starts_at := none, ends_at := some 2000, customer_id := some "c1" }]
    exchange_rates := []
    groups := [{ group_id := "g", priority := 0, mode := .Stackable }]
    promotions := [] }

def holds (r : Rule) : Bool := ruleHolds r (promo [r]) input

-- Minimum order total: subtotal is 4499
#guard holds (.OrderTotalRange (some (eur 5000)) none) = false
#guard holds (.OrderTotalRange (some (eur 4499)) (some (eur 4499))) = true
-- First order only
#guard holds (.MaxCompletedOrders 0) = false
#guard holds (.MaxCompletedOrders 1) = true
-- Usage limit: P1 used twice
#guard holds (.MaxUsesPerCustomer 2) = false
#guard holds (.MaxUsesPerCustomer 3) = true
-- Check-in window
#guard holds (.AttributeDateWindow "checkin" (some 100) (some 200)) = true
#guard holds (.AttributeDateWindow "checkin" (some 100) (some 150)) = false
#guard holds (.AttributeDateWindow "checkout" (some 100) (some 200)) = false
#guard (ruleHolds (.AttributeDateWindow "checkin" none none) (promo [])
          { input with cart := { cart with attributes := [{ key := "checkin", value := "soon" }] } }) = false
-- Tags and attributes
#guard holds (.TagsMatch ["#ios"] [] ["#partner"]) = true
#guard holds (.TagsMatch [] ["#android", "#ios"] []) = true
#guard holds (.TagsMatch [] ["#android"] []) = false
#guard holds (.TagsMatch [] [] ["#promo"]) = false
#guard holds (.AttributeIn "hotel_kind" ["resort", "hostel"]) = true
#guard holds (.AttributeIn "hotel_kind" ["hostel"]) = false
-- Coupons
#guard holds .RequiresCoupon = true
#guard (ruleHolds .RequiresCoupon (promo []) { input with now := 2000 }) = false
#guard (ruleHolds .RequiresCoupon (promo []) { input with customer := { customer with customer_id := some "c2" } }) = false
#guard (ruleHolds .RequiresCoupon (promo []) { input with cart := { cart with coupon_codes := ["NOPE"] } }) = false
-- parseDecimal
#guard parseDecimal "150" = some 150
#guard parseDecimal "-7" = some (-7)
#guard parseDecimal "+5" = none
#guard parseDecimal "" = none
#guard parseDecimal "1234567890123456789" = none

end Discount.Examples
