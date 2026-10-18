import Discount.Validate.Checks

/-!
# Catalogue checks (ADR 6): rates, coupons, order cap, actions, rules, promotions
-/
namespace Discount

def checkRate (r : ExchangeRate) (k : Nat) : Except PricingError Unit :=
  if !validCurrency r.«from» then .error (.InvalidCurrency s!"exchange_rates[{k}].from")
  else if !validCurrency r.to then .error (.InvalidCurrency s!"exchange_rates[{k}].to")
  else if !ratePartInRange r.numerator then .error (.InvalidExchangeRate s!"exchange_rates[{k}].numerator")
  else if !ratePartInRange r.denominator then .error (.InvalidExchangeRate s!"exchange_rates[{k}].denominator")
  else .ok ()

def checkRatesEach : List ExchangeRate → Nat → Except PricingError Unit
  | [], _ => .ok ()
  | r :: rs, k => andThen (checkRate r k) (checkRatesEach rs (k + 1))

/-- First `(from, to)` pair seen twice. -/
def firstDuplicatePair : List ExchangeRate → List (Currency × Currency) → Option (Currency × Currency)
  | [], _ => none
  | r :: rs, seen =>
    if (r.«from», r.to) ∈ seen then some (r.«from», r.to) else firstDuplicatePair rs ((r.«from», r.to) :: seen)

def checkRates (rates : List ExchangeRate) : Except PricingError Unit :=
  andThen (checkRatesEach rates 0)
  (match firstDuplicatePair rates [] with
    | some (f, t) => .error (.DuplicateExchangeRate f t)
    | none => .ok ())

def checkCouponsEach : List Coupon → Nat → Except PricingError Unit
  | [], _ => .ok ()
  | c :: cs, k =>
    if c.uses < 0 then .error (.InvalidCoupon s!"coupons[{k}].uses")
    else match c.max_uses with
      | some m => if m < 0 then .error (.InvalidCoupon s!"coupons[{k}].max_uses") else checkCouponsEach cs (k + 1)
      | none => checkCouponsEach cs (k + 1)

/-- Per-coupon counters, then duplicate codes. -/
def checkCoupons (coupons : List Coupon) : Except PricingError Unit :=
  andThen (checkCouponsEach coupons 0)
    (checkDuplicates (coupons.map (·.code)) PricingError.DuplicateCouponCode)

def checkOrderCap (oc : OrderCap) (cartCurrency : Currency) (rates : List ExchangeRate) : Except PricingError Unit :=
  andThen (checkOptMoney oc.amount cartCurrency rates "policy.order_cap.amount")
    (match oc.bps with
      | none => .ok ()
      | some b => if !bpsInRange b then .error (.InvalidBasisPoints "policy.order_cap.bps") else .ok ())

def checkAction (a : Action) (cartCurrency : Currency) (rates : List ExchangeRate) (i : Nat) : Except PricingError Unit :=
  match a with
  | .OrderPercentage b =>
      if !bpsInRange b then .error (.InvalidBasisPoints s!"promotions[{i}].action.bps") else .ok ()
  | .OrderFixedAmount m => checkMoney m cartCurrency rates s!"promotions[{i}].action.amount"

def checkRule (r : Rule) (cartCurrency : Currency) (rates : List ExchangeRate) (i k : Nat) : Except PricingError Unit :=
  match r with
  | .OrderTotalRange mn mx =>
      andThen (checkOptMoney mn cartCurrency rates s!"promotions[{i}].rules[{k}].min")
        (checkOptMoney mx cartCurrency rates s!"promotions[{i}].rules[{k}].max")
  | .MaxCompletedOrders m => if m < 0 then .error (.InvalidRule s!"promotions[{i}].rules[{k}].max") else .ok ()
  | .MaxUsesPerCustomer m => if m < 0 then .error (.InvalidRule s!"promotions[{i}].rules[{k}].max") else .ok ()
  | .MaxUsesTotal m => if m < 0 then .error (.InvalidRule s!"promotions[{i}].rules[{k}].max") else .ok ()
  | _ => .ok ()

def checkRules : List Rule → Currency → List ExchangeRate → Nat → Nat → Except PricingError Unit
  | [], _, _, _, _ => .ok ()
  | r :: rs, cur, rates, i, k => andThen (checkRule r cur rates i k) (checkRules rs cur rates i (k + 1))

def checkPromotions : List Promotion → Currency → List ExchangeRate → List GroupId → Nat → Except PricingError Unit
  | [], _, _, _, _ => .ok ()
  | p :: ps, cur, rates, gids, i =>
    andThen (checkAction p.action cur rates i)
    (andThen (checkOptMoney p.cap cur rates s!"promotions[{i}].cap")
    (andThen (checkOptMoney p.remaining_budget cur rates s!"promotions[{i}].remaining_budget")
    (andThen (if p.uses_total < 0 then .error (.InvalidPromotion s!"promotions[{i}].uses_total") else .ok ())
    (andThen (checkRules p.rules cur rates i 0)
    (andThen (if p.group_id ∈ gids then .ok () else .error (.UnknownGroup i))
    (checkPromotions ps cur rates gids (i + 1)))))))

end Discount
