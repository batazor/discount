import Discount.Eligibility

/-!
# Invariant 7: eligibility depends only on `now`, the cart, the customer and the coupons
-/
namespace Discount

/-! ## 7. Eligibility depends only on its inputs -/

theorem couponValid_congr (i j : PricingInput) (h1 : i.now = j.now) (h2 : i.cart = j.cart)
    (h3 : i.customer = j.customer) (pid : PromotionId) (c : Coupon) :
    couponValid i pid c = couponValid j pid c := by
  simp [couponValid, h1, h2, h3]

theorem ruleHolds_congr (r : Rule) (p : Promotion) (i j : PricingInput) (h1 : i.now = j.now)
    (h2 : i.cart = j.cart) (h3 : i.customer = j.customer) (h4 : i.coupons = j.coupons) :
    ruleHolds r p i = ruleHolds r p j := by
  have hc : couponValid i p.promotion_id = couponValid j p.promotion_id := by
    funext c; exact couponValid_congr i j h1 h2 h3 p.promotion_id c
  cases r <;> simp only [ruleHolds, h1, h2, h3, h4, hc]

theorem firstFailingRule_congr (rs : List Rule) (p : Promotion) (i j : PricingInput) (h1 : i.now = j.now)
    (h2 : i.cart = j.cart) (h3 : i.customer = j.customer) (h4 : i.coupons = j.coupons) (k : Nat) :
    firstFailingRule rs p i k = firstFailingRule rs p j k := by
  induction rs generalizing k with
  | nil => rfl
  | cons r rs ih => simp [firstFailingRule, ruleHolds_congr r p i j h1 h2 h3 h4, ih]

/-- Eligibility reads nothing but `now`, the cart, the customer context and the coupons. -/
theorem eligible_depends_on_input (p : Promotion) (i j : PricingInput) (h1 : i.now = j.now)
    (h2 : i.cart = j.cart) (h3 : i.customer = j.customer) (h4 : i.coupons = j.coupons) :
    eligible p i = eligible p j := by
  simp [eligible, firstFailingRule_congr p.rules p i j h1 h2 h3 h4]

end Discount
