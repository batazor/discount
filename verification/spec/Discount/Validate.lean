import Discount.Types
import Discount.Rounding

/-!
# validate (ADR 6)

Sequential checks in the normative order of `docs/ubiquitous-language.md`. The first
failure is reported. `andThen` sequences two checks; both operands are evaluated (pure),
only the first error survives. Money in another currency than the cart's is checked
through `convertAmount` (OpenSpec `multi-currency`).
-/
namespace Discount

def validCurrency (c : Currency) : Bool :=
  c.length == 3 && c.toList.all (fun ch => 'A' ≤ ch && ch ≤ 'Z')

def amountInRange (a : Int) : Bool := 0 ≤ a && a ≤ MAX_MINOR_UNITS

def bpsInRange (b : BasisPoints) : Bool := 0 ≤ b && b ≤ BPS_SCALE

def ratePartInRange (x : Int) : Bool := 1 ≤ x && x ≤ MAX_RATE_PART

def andThen (a b : Except PricingError Unit) : Except PricingError Unit :=
  match a with
  | .error e => .error e
  | .ok () => b

/-- The rate converting `from` into `to`, if listed. -/
def findRate (rates : List ExchangeRate) (from_ to : Currency) : Option ExchangeRate :=
  rates.find? (fun r => r.«from» == from_ && r.to == to)

/-- `amount` of currency `from` in currency `to`: unchanged for the same currency, else
`roundDiv Down (amount * numerator) denominator`; `none` without a rate. -/
def convertAmount (rates : List ExchangeRate) (amount : Int) (from_ to : Currency) : Option Int :=
  if from_ == to then some amount
  else match findRate rates from_ to with
    | some r => some (roundDiv .Down (amount * r.numerator) r.denominator)
    | none => none

/-- `InvalidCurrency`, `CurrencyMismatch`, `AmountOutOfRange` (original, then converted). -/
def checkMoney (m : Money) (cartCurrency : Currency) (rates : List ExchangeRate) (field : String) :
    Except PricingError Unit :=
  if !validCurrency m.currency then .error (.InvalidCurrency field)
  else match convertAmount rates m.amount m.currency cartCurrency with
    | none => .error (.CurrencyMismatch field)
    | some c =>
      if !amountInRange m.amount then .error (.AmountOutOfRange field)
      else if !amountInRange c then .error (.AmountOutOfRange field)
      else .ok ()

def checkOptMoney (m : Option Money) (cartCurrency : Currency) (rates : List ExchangeRate) (field : String) :
    Except PricingError Unit :=
  match m with
  | none => .ok ()
  | some m => checkMoney m cartCurrency rates field

def checkLines : List CartLine → Currency → Nat → Except PricingError Unit
  | [], _, _ => .ok ()
  | l :: ls, cur, i =>
    let field := s!"cart.lines[{i}].unit_price"
    if !validCurrency l.unit_price.currency then .error (.InvalidCurrency field)
    else if l.unit_price.currency != cur then .error (.MixedCurrencies i)
    else if !amountInRange l.unit_price.amount then .error (.AmountOutOfRange field)
    else if !(1 ≤ l.quantity && l.quantity ≤ MAX_QUANTITY) then .error (.InvalidQuantity i)
    else checkLines ls cur (i + 1)

/-- First element seen twice, scanning in order. -/
def firstDuplicate : List String → List String → Option String
  | [], _ => none
  | x :: xs, seen => if x ∈ seen then some x else firstDuplicate xs (x :: seen)

def checkDuplicates (ids : List String) (mk : String → PricingError) : Except PricingError Unit :=
  match firstDuplicate ids [] with
  | some x => .error (mk x)
  | none => .ok ()

/-- Index of the second occurrence of a repeated element, if any. -/
def secondOccurrence : List String → List String → Nat → Option Nat
  | [], _, _ => none
  | x :: xs, seen, k => if x ∈ seen then some k else secondOccurrence xs (x :: seen) (k + 1)

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
