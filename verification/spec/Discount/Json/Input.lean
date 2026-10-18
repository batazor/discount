import Discount.Json.Codec

/-!
# JSON codecs of the input types (`PricingInput` and everything in it)
-/
namespace Discount

open Lean (Json ToJson FromJson toJson fromJson?)

-- Money --------------------------------------------------------------------

instance : ToJson Money where
  toJson m := Json.mkObj [("amount", toJson m.amount), ("currency", toJson m.currency)]

instance : FromJson Money where
  fromJson? j := do
    checkKeys j ["amount", "currency"]
    pure { amount := ← getField j "amount", currency := ← getField j "currency" }

-- Policies -----------------------------------------------------------------

instance : ToJson RoundingPolicy where
  toJson
    | .HalfUp => variant "HalfUp"
    | .HalfEven => variant "HalfEven"
    | .Down => variant "Down"

instance : FromJson RoundingPolicy where
  fromJson? j := do
    checkKeys j ["type"]
    match ← getVariant j with
    | "HalfUp" => pure .HalfUp
    | "HalfEven" => pure .HalfEven
    | "Down" => pure .Down
    | v => throw s!"unknown RoundingPolicy '{v}'"

instance : ToJson StackingPolicy where
  toJson
    | .Sequential => variant "Sequential"
    | .Additive => variant "Additive"

instance : FromJson StackingPolicy where
  fromJson? j := do
    checkKeys j ["type"]
    match ← getVariant j with
    | "Sequential" => pure .Sequential
    | "Additive" => pure .Additive
    | v => throw s!"unknown StackingPolicy '{v}'"

instance : ToJson OrderCap where
  toJson oc := Json.mkObj [("amount", optJson oc.amount), ("bps", optJson oc.bps)]

instance : FromJson OrderCap where
  fromJson? j := do
    checkKeys j ["amount", "bps"]
    pure { amount := ← getOpt j "amount", bps := ← getOpt j "bps" }

instance : ToJson PricingPolicy where
  toJson p := Json.mkObj [("rounding", toJson p.rounding), ("stacking", toJson p.stacking),
    ("order_cap", toJson p.order_cap)]

instance : FromJson PricingPolicy where
  fromJson? j := do
    checkKeys j ["rounding", "stacking", "order_cap"]
    pure { rounding := ← getField j "rounding", stacking := ← getField j "stacking",
           order_cap := ← getField j "order_cap" }

-- Cart ---------------------------------------------------------------------

instance : ToJson CartLine where
  toJson l := Json.mkObj [("line_id", toJson l.line_id), ("unit_price", toJson l.unit_price),
    ("quantity", toJson l.quantity)]

instance : FromJson CartLine where
  fromJson? j := do
    checkKeys j ["line_id", "unit_price", "quantity"]
    pure { line_id := ← getField j "line_id", unit_price := ← getField j "unit_price",
           quantity := ← getField j "quantity" }

instance : ToJson Attribute where
  toJson a := Json.mkObj [("key", toJson a.key), ("value", toJson a.value)]

instance : FromJson Attribute where
  fromJson? j := do
    checkKeys j ["key", "value"]
    pure { key := ← getField j "key", value := ← getField j "value" }

instance : ToJson CartSnapshot where
  toJson c := Json.mkObj [("currency", toJson c.currency), ("lines", toJson c.lines),
    ("tags", toJson c.tags), ("attributes", toJson c.attributes), ("coupon_codes", toJson c.coupon_codes)]

instance : FromJson CartSnapshot where
  fromJson? j := do
    checkKeys j ["currency", "lines", "tags", "attributes", "coupon_codes"]
    pure { currency := ← getField j "currency", lines := ← getField j "lines",
           tags := ← getField j "tags", attributes := ← getField j "attributes",
           coupon_codes := ← getField j "coupon_codes" }

instance : ToJson PromotionUse where
  toJson u := Json.mkObj [("promotion_id", toJson u.promotion_id), ("count", toJson u.count)]

instance : FromJson PromotionUse where
  fromJson? j := do
    checkKeys j ["promotion_id", "count"]
    pure { promotion_id := ← getField j "promotion_id", count := ← getField j "count" }

instance : ToJson CustomerContext where
  toJson c := Json.mkObj [("customer_id", optJson c.customer_id),
    ("completed_orders", toJson c.completed_orders), ("uses", toJson c.uses)]

instance : FromJson CustomerContext where
  fromJson? j := do
    checkKeys j ["customer_id", "completed_orders", "uses"]
    pure { customer_id := ← getOpt j "customer_id", completed_orders := ← getField j "completed_orders",
           uses := ← getField j "uses" }

instance : ToJson Coupon where
  toJson c := Json.mkObj [("code", toJson c.code), ("promotion_id", toJson c.promotion_id),
    ("starts_at", optJson c.starts_at), ("ends_at", optJson c.ends_at), ("customer_id", optJson c.customer_id),
    ("max_uses", optJson c.max_uses), ("uses", toJson c.uses)]

instance : FromJson Coupon where
  fromJson? j := do
    checkKeys j ["code", "promotion_id", "starts_at", "ends_at", "customer_id", "max_uses", "uses"]
    pure { code := ← getField j "code", promotion_id := ← getField j "promotion_id",
           starts_at := ← getOpt j "starts_at", ends_at := ← getOpt j "ends_at",
           customer_id := ← getOpt j "customer_id", max_uses := ← getOpt j "max_uses",
           uses := ← getField j "uses" }

instance : ToJson ExchangeRate where
  toJson r := Json.mkObj [("from", toJson r.«from»), ("to", toJson r.to),
    ("numerator", toJson r.numerator), ("denominator", toJson r.denominator)]

instance : FromJson ExchangeRate where
  fromJson? j := do
    checkKeys j ["from", "to", "numerator", "denominator"]
    pure { «from» := ← getField j "from", to := ← getField j "to",
           numerator := ← getField j "numerator", denominator := ← getField j "denominator" }

-- Groups -------------------------------------------------------------------

instance : ToJson StackingMode where
  toJson
    | .Stackable => variant "Stackable"
    | .FirstMatch => variant "FirstMatch"

instance : FromJson StackingMode where
  fromJson? j := do
    checkKeys j ["type"]
    match ← getVariant j with
    | "Stackable" => pure .Stackable
    | "FirstMatch" => pure .FirstMatch
    | v => throw s!"unknown StackingMode '{v}'"

instance : ToJson StackingGroup where
  toJson g := Json.mkObj [("group_id", toJson g.group_id), ("priority", toJson g.priority),
    ("mode", toJson g.mode)]

instance : FromJson StackingGroup where
  fromJson? j := do
    checkKeys j ["group_id", "priority", "mode"]
    pure { group_id := ← getField j "group_id", priority := ← getField j "priority",
           mode := ← getField j "mode" }

-- Promotion ----------------------------------------------------------------

instance : ToJson Rule where
  toJson
    | .DateWindow s e => variant "DateWindow" [("starts_at", optJson s), ("ends_at", optJson e)]
    | .OrderTotalRange mn mx => variant "OrderTotalRange" [("min", optJson mn), ("max", optJson mx)]
    | .TagsMatch a b c => variant "TagsMatch" [("all", toJson a), ("any", toJson b), ("none", toJson c)]
    | .AttributeIn k vs => variant "AttributeIn" [("key", toJson k), ("values", toJson vs)]
    | .AttributeDateWindow k s e =>
        variant "AttributeDateWindow" [("key", toJson k), ("starts_at", optJson s), ("ends_at", optJson e)]
    | .MaxCompletedOrders m => variant "MaxCompletedOrders" [("max", toJson m)]
    | .MaxUsesPerCustomer m => variant "MaxUsesPerCustomer" [("max", toJson m)]
    | .MaxUsesTotal m => variant "MaxUsesTotal" [("max", toJson m)]
    | .RequiresCoupon => variant "RequiresCoupon"

instance : FromJson Rule where
  fromJson? j := do
    match ← getVariant j with
    | "DateWindow" =>
        checkKeys j ["type", "starts_at", "ends_at"]
        pure (.DateWindow (← getOpt j "starts_at") (← getOpt j "ends_at"))
    | "OrderTotalRange" =>
        checkKeys j ["type", "min", "max"]
        pure (.OrderTotalRange (← getOpt j "min") (← getOpt j "max"))
    | "TagsMatch" =>
        checkKeys j ["type", "all", "any", "none"]
        pure (.TagsMatch (← getField j "all") (← getField j "any") (← getField j "none"))
    | "AttributeIn" =>
        checkKeys j ["type", "key", "values"]
        pure (.AttributeIn (← getField j "key") (← getField j "values"))
    | "AttributeDateWindow" =>
        checkKeys j ["type", "key", "starts_at", "ends_at"]
        pure (.AttributeDateWindow (← getField j "key") (← getOpt j "starts_at") (← getOpt j "ends_at"))
    | "MaxCompletedOrders" =>
        checkKeys j ["type", "max"]
        pure (.MaxCompletedOrders (← getField j "max"))
    | "MaxUsesPerCustomer" =>
        checkKeys j ["type", "max"]
        pure (.MaxUsesPerCustomer (← getField j "max"))
    | "MaxUsesTotal" =>
        checkKeys j ["type", "max"]
        pure (.MaxUsesTotal (← getField j "max"))
    | "RequiresCoupon" =>
        checkKeys j ["type"]
        pure .RequiresCoupon
    | v => throw s!"unknown Rule '{v}'"

instance : ToJson Action where
  toJson
    | .OrderPercentage b => variant "OrderPercentage" [("bps", toJson b)]
    | .OrderFixedAmount m => variant "OrderFixedAmount" [("amount", toJson m)]

instance : FromJson Action where
  fromJson? j := do
    match ← getVariant j with
    | "OrderPercentage" =>
        checkKeys j ["type", "bps"]
        pure (.OrderPercentage (← getField j "bps"))
    | "OrderFixedAmount" =>
        checkKeys j ["type", "amount"]
        pure (.OrderFixedAmount (← getField j "amount"))
    | v => throw s!"unknown Action '{v}'"

instance : ToJson Promotion where
  toJson p := Json.mkObj [
    ("promotion_id", toJson p.promotion_id), ("group_id", toJson p.group_id),
    ("priority", toJson p.priority), ("exclusive", toJson p.exclusive),
    ("cap", optJson p.cap), ("remaining_budget", optJson p.remaining_budget),
    ("uses_total", toJson p.uses_total),
    ("rounding", optJson p.rounding), ("rules", toJson p.rules), ("action", toJson p.action)]

instance : FromJson Promotion where
  fromJson? j := do
    checkKeys j ["promotion_id", "group_id", "priority", "exclusive", "cap",
      "remaining_budget", "uses_total", "rounding", "rules", "action"]
    pure { promotion_id := ← getField j "promotion_id", group_id := ← getField j "group_id",
           priority := ← getField j "priority", exclusive := ← getField j "exclusive",
           cap := ← getOpt j "cap", remaining_budget := ← getOpt j "remaining_budget",
           uses_total := ← getField j "uses_total",
           rounding := ← getOpt j "rounding", rules := ← getField j "rules",
           action := ← getField j "action" }

instance : ToJson PricingInput where
  toJson i := Json.mkObj [("now", toJson i.now), ("policy", toJson i.policy),
    ("customer", toJson i.customer), ("cart", toJson i.cart), ("coupons", toJson i.coupons),
    ("exchange_rates", toJson i.exchange_rates), ("groups", toJson i.groups),
    ("promotions", toJson i.promotions)]

instance : FromJson PricingInput where
  fromJson? j := do
    checkKeys j ["now", "policy", "customer", "cart", "coupons", "exchange_rates", "groups", "promotions"]
    pure { now := ← getField j "now", policy := ← getField j "policy",
           customer := ← getField j "customer", cart := ← getField j "cart",
           coupons := ← getField j "coupons", exchange_rates := ← getField j "exchange_rates",
           groups := ← getField j "groups", promotions := ← getField j "promotions" }

end Discount
