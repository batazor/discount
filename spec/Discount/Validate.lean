import Discount.Types

/-!
# validate (ADR 6)

Sequential checks in the normative order of `docs/ubiquitous-language.md`. The first
failure is reported. `andThen` sequences two checks; both operands are evaluated (pure),
only the first error survives.
-/
namespace Discount

def validCurrency (c : Currency) : Bool :=
  c.length == 3 && c.toList.all (fun ch => 'A' ≤ ch && ch ≤ 'Z')

def amountInRange (a : Int) : Bool := 0 ≤ a && a ≤ MAX_MINOR_UNITS

def bpsInRange (b : BasisPoints) : Bool := 0 ≤ b && b ≤ BPS_SCALE

def andThen (a b : Except PricingError Unit) : Except PricingError Unit :=
  match a with
  | .error e => .error e
  | .ok () => b

/-- `InvalidCurrency`, `CurrencyMismatch`, `AmountOutOfRange` for one `Money` field. -/
def checkMoney (m : Money) (cartCurrency : Currency) (field : String) : Except PricingError Unit :=
  if !validCurrency m.currency then .error (.InvalidCurrency field)
  else if m.currency != cartCurrency then .error (.CurrencyMismatch field)
  else if !amountInRange m.amount then .error (.AmountOutOfRange field)
  else .ok ()

def checkOptMoney (m : Option Money) (cartCurrency : Currency) (field : String) : Except PricingError Unit :=
  match m with
  | none => .ok ()
  | some m => checkMoney m cartCurrency field

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

def checkOrderCap (oc : OrderCap) (cartCurrency : Currency) : Except PricingError Unit :=
  andThen (checkOptMoney oc.amount cartCurrency "policy.order_cap.amount")
    (match oc.bps with
      | none => .ok ()
      | some b => if !bpsInRange b then .error (.InvalidBasisPoints "policy.order_cap.bps") else .ok ())

def checkAction (a : Action) (cartCurrency : Currency) (i : Nat) : Except PricingError Unit :=
  match a with
  | .OrderPercentage b =>
      if !bpsInRange b then .error (.InvalidBasisPoints s!"promotions[{i}].action.bps") else .ok ()
  | .OrderFixedAmount m => checkMoney m cartCurrency s!"promotions[{i}].action.amount"

def checkPromotions : List Promotion → Currency → List GroupId → Nat → Except PricingError Unit
  | [], _, _, _ => .ok ()
  | p :: ps, cur, gids, i =>
    andThen (checkAction p.action cur i)
    (andThen (checkOptMoney p.cap cur s!"promotions[{i}].cap")
    (andThen (checkOptMoney p.remaining_budget cur s!"promotions[{i}].remaining_budget")
    (andThen (if p.group_id ∈ gids then .ok () else .error (.UnknownGroup i))
    (checkPromotions ps cur gids (i + 1)))))

def checkDuplicates (ids : List String) (mk : String → PricingError) : Except PricingError Unit :=
  match firstDuplicate ids [] with
  | some x => .error (mk x)
  | none => .ok ()

def validate (i : PricingInput) : Except PricingError Unit :=
  andThen (if !validCurrency i.cart.currency then .error (.InvalidCurrency "cart.currency") else .ok ())
  (andThen (checkLines i.cart.lines i.cart.currency 0)
  (andThen (checkDuplicates (i.cart.lines.map (·.line_id)) PricingError.DuplicateLineId)
  (andThen (if orderSubtotal i.cart > MAX_MINOR_UNITS then .error (.AmountOutOfRange "cart.subtotal") else .ok ())
  (andThen (checkOrderCap i.policy.order_cap i.cart.currency)
  (andThen (checkDuplicates (i.groups.map (·.group_id)) PricingError.DuplicateGroupId)
  (andThen (checkPromotions i.promotions i.cart.currency (i.groups.map (·.group_id)) 0)
  (checkDuplicates (i.promotions.map (·.promotion_id)) PricingError.DuplicatePromotionId)))))))

end Discount
