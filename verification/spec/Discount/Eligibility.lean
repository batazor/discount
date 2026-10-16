import Discount.Types

/-!
# eligible (ADR 7, OpenSpec `eligibility-rules`, `coupons`)

A promotion is eligible when every rule holds. `firstFailingRule` reports the index of the
first rule that does not hold, for `SkipReason.NotEligible`. Rules read `(now, cart,
customer, coupons)` and the promotion; amounts are already in the cart currency
(`normalize` ran before).
-/
namespace Discount

/-- Half-open window `[starts_at, ends_at)` on `t`, each bound optional. -/
def inWindow (starts_at ends_at : Option Timestamp) (t : Int) : Bool :=
  (match starts_at with | none => true | some s => decide (s ≤ t)) &&
  (match ends_at with | none => true | some e => decide (t < e))

/-- Optional leading `-`, then 1 to 18 ASCII digits. Anything else is `none`. The same
function exists in Rust; both reject what `i64` could not hold. -/
def parseDecimal (s : String) : Option Int :=
  let (neg, digits) := match s.toList with
    | '-' :: rest => (true, rest)
    | cs => (false, cs)
  if digits.isEmpty || digits.length > 18 || !digits.all Char.isDigit then none
  else
    let n : Int := digits.foldl (fun acc c => acc * 10 + (c.toNat - '0'.toNat : Nat)) 0
    some (if neg then -n else n)

def usesOf (c : CustomerContext) (pid : PromotionId) : Int :=
  match c.uses.find? (fun u => u.promotion_id == pid) with
  | some u => u.count
  | none => 0

/-- Remaining uses: `max_uses` absent, or `uses < max_uses`. -/
def couponHasUses (c : Coupon) : Bool :=
  match c.max_uses with
  | none => true
  | some m => decide (c.uses < m)

def couponValid (i : PricingInput) (pid : PromotionId) (c : Coupon) : Bool :=
  c.promotion_id == pid && i.cart.coupon_codes.contains c.code &&
    inWindow c.starts_at c.ends_at i.now &&
    (match c.customer_id with | none => true | some cid => i.customer.customer_id == some cid) &&
    couponHasUses c

def ruleHolds (r : Rule) (p : Promotion) (i : PricingInput) : Bool :=
  match r with
  | .DateWindow starts_at ends_at => inWindow starts_at ends_at i.now
  | .OrderTotalRange mn mx =>
      let s := orderSubtotal i.cart
      (match mn with | none => true | some m => decide (m.amount ≤ s)) &&
      (match mx with | none => true | some m => decide (s ≤ m.amount))
  | .TagsMatch alls anys nones =>
      alls.all (fun t => i.cart.tags.contains t) &&
      (anys.isEmpty || anys.any (fun t => i.cart.tags.contains t)) &&
      nones.all (fun t => !i.cart.tags.contains t)
  | .AttributeIn key values =>
      i.cart.attributes.any (fun a => a.key == key && values.contains a.value)
  | .AttributeDateWindow key starts_at ends_at =>
      match i.cart.attributes.find? (fun a => a.key == key) with
      | none => false
      | some a =>
        match parseDecimal a.value with
        | none => false
        | some t => inWindow starts_at ends_at t
  | .MaxCompletedOrders m => decide (i.customer.completed_orders ≤ m)
  | .MaxUsesPerCustomer m => decide (usesOf i.customer p.promotion_id < m)
  | .MaxUsesTotal m => decide (p.uses_total < m)
  | .RequiresCoupon => i.coupons.any (couponValid i p.promotion_id)

def firstFailingRule : List Rule → Promotion → PricingInput → Nat → Option Nat
  | [], _, _, _ => none
  | r :: rs, p, i, k => if ruleHolds r p i then firstFailingRule rs p i (k + 1) else some k

def eligible (p : Promotion) (i : PricingInput) : Bool :=
  (firstFailingRule p.rules p i 0).isNone

end Discount
