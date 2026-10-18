import Discount.Validate.Checks

/-!
# Cart and customer checks (ADR 6)
-/
namespace Discount

def checkLines : List CartLine → Currency → Nat → Except PricingError Unit
  | [], _, _ => .ok ()
  | l :: ls, cur, i =>
    let field := s!"cart.lines[{i}].unit_price"
    if !validCurrency l.unit_price.currency then .error (.InvalidCurrency field)
    else if l.unit_price.currency != cur then .error (.MixedCurrencies i)
    else if !amountInRange l.unit_price.amount then .error (.AmountOutOfRange field)
    else if !(1 ≤ l.quantity && l.quantity ≤ MAX_QUANTITY) then .error (.InvalidQuantity i)
    else checkLines ls cur (i + 1)

def checkUses : List PromotionUse → Nat → Except PricingError Unit
  | [], _ => .ok ()
  | u :: us, k =>
    if u.count < 0 then .error (.InvalidCustomerContext s!"customer.uses[{k}].count")
    else checkUses us (k + 1)

def checkCustomer (c : CustomerContext) : Except PricingError Unit :=
  andThen (if c.completed_orders < 0 then .error (.InvalidCustomerContext "customer.completed_orders") else .ok ())
  (andThen (checkUses c.uses 0)
  (match secondOccurrence (c.uses.map (·.promotion_id)) [] 0 with
    | some k => .error (.InvalidCustomerContext s!"customer.uses[{k}]")
    | none => .ok ()))

end Discount
