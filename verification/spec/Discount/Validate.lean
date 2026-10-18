import Discount.Validate.Cart
import Discount.Validate.Catalogue

/-!
# validate (ADR 6)

Sequential checks in the normative order of `docs/ubiquitous-language.md`. The first failure
is reported. The checks themselves live in `Validate/Checks`, `Validate/Cart` and
`Validate/Catalogue`.
-/
namespace Discount

def validate (i : PricingInput) : Except PricingError Unit :=
  let cur := i.cart.currency
  let rates := i.exchange_rates
  andThen (if !validCurrency cur then .error (.InvalidCurrency "cart.currency") else .ok ())
  (andThen (checkLines i.cart.lines cur 0)
  (andThen (checkDuplicates (i.cart.lines.map (·.line_id)) PricingError.DuplicateLineId)
  (andThen (checkDuplicates i.cart.coupon_codes PricingError.DuplicateCouponCode)
  (andThen (if orderSubtotal i.cart > MAX_MINOR_UNITS then .error (.AmountOutOfRange "cart.subtotal") else .ok ())
  (andThen (checkCustomer i.customer)
  (andThen (checkRates rates)
  (andThen (checkCoupons i.coupons)
  (andThen (checkOrderCap i.policy.order_cap cur rates)
  (andThen (checkDuplicates (i.groups.map (·.group_id)) PricingError.DuplicateGroupId)
  (andThen (checkPromotions i.promotions cur rates (i.groups.map (·.group_id)) 0)
  (checkDuplicates (i.promotions.map (·.promotion_id)) PricingError.DuplicatePromotionId)))))))))))

end Discount
