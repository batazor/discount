import Lean.Data.Json
import Discount.Types

/-!
# JSON contract (docs/ubiquitous-language.md)

Handwritten encoders and decoders so that the wire format is exactly the documented one:
enums as `{"type": "<Variant>", ...fields}`, optional fields as `null`, extra keys
rejected. Output key order is alphabetical (Lean's `Json.obj` is a sorted map); the DRT
compares parsed values, not bytes.
-/
namespace Discount

open Lean (Json ToJson FromJson toJson fromJson?)

def objKeys : Json → List String
  | .obj kvs => kvs.keys
  | _ => []

/-- Fail if `j` is not an object or carries a key outside `allowed`. -/
def checkKeys (j : Json) (allowed : List String) : Except String Unit :=
  match j with
  | .obj _ =>
      match (objKeys j).find? (fun k => !(allowed.contains k)) with
      | some k => .error s!"unexpected key '{k}'"
      | none => .ok ()
  | _ => .error "expected object"

def getField [FromJson α] (j : Json) (k : String) : Except String α := do
  let v ← j.getObjVal? k
  fromJson? v

/-- Missing key and `null` both read as `none`. -/
def getOpt [FromJson α] (j : Json) (k : String) : Except String (Option α) :=
  match j.getObjVal? k with
  | .error _ => .ok none
  | .ok .null => .ok none
  | .ok v => some <$> fromJson? v

def optJson [ToJson α] : Option α → Json
  | none => .null
  | some a => toJson a

def variant (name : String) (fields : List (String × Json) := []) : Json :=
  Json.mkObj (("type", Json.str name) :: fields)

def getVariant (j : Json) : Except String String := getField j "type"

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

instance : ToJson CartSnapshot where
  toJson c := Json.mkObj [("currency", toJson c.currency), ("lines", toJson c.lines)]

instance : FromJson CartSnapshot where
  fromJson? j := do
    checkKeys j ["currency", "lines"]
    pure { currency := ← getField j "currency", lines := ← getField j "lines" }

instance : ToJson CustomerContext where
  toJson _ := Json.mkObj []

instance : FromJson CustomerContext where
  fromJson? j := do
    checkKeys j []
    pure {}

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

instance : FromJson Rule where
  fromJson? j := do
    match ← getVariant j with
    | "DateWindow" =>
        checkKeys j ["type", "starts_at", "ends_at"]
        pure (.DateWindow (← getOpt j "starts_at") (← getOpt j "ends_at"))
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
    ("rounding", optJson p.rounding), ("rules", toJson p.rules), ("action", toJson p.action)]

instance : FromJson Promotion where
  fromJson? j := do
    checkKeys j ["promotion_id", "group_id", "priority", "exclusive", "cap",
      "remaining_budget", "rounding", "rules", "action"]
    pure { promotion_id := ← getField j "promotion_id", group_id := ← getField j "group_id",
           priority := ← getField j "priority", exclusive := ← getField j "exclusive",
           cap := ← getOpt j "cap", remaining_budget := ← getOpt j "remaining_budget",
           rounding := ← getOpt j "rounding", rules := ← getField j "rules",
           action := ← getField j "action" }

instance : ToJson PricingInput where
  toJson i := Json.mkObj [("now", toJson i.now), ("policy", toJson i.policy),
    ("customer", toJson i.customer), ("cart", toJson i.cart), ("groups", toJson i.groups),
    ("promotions", toJson i.promotions)]

instance : FromJson PricingInput where
  fromJson? j := do
    checkKeys j ["now", "policy", "customer", "cart", "groups", "promotions"]
    pure { now := ← getField j "now", policy := ← getField j "policy",
           customer := ← getField j "customer", cart := ← getField j "cart",
           groups := ← getField j "groups", promotions := ← getField j "promotions" }

-- Errors -------------------------------------------------------------------

instance : ToJson PricingError where
  toJson
    | .InvalidCurrency f => variant "InvalidCurrency" [("field", toJson f)]
    | .MixedCurrencies i => variant "MixedCurrencies" [("line_index", toJson i)]
    | .CurrencyMismatch f => variant "CurrencyMismatch" [("field", toJson f)]
    | .AmountOutOfRange f => variant "AmountOutOfRange" [("field", toJson f)]
    | .InvalidBasisPoints f => variant "InvalidBasisPoints" [("field", toJson f)]
    | .InvalidQuantity i => variant "InvalidQuantity" [("line_index", toJson i)]
    | .DuplicateLineId x => variant "DuplicateLineId" [("line_id", toJson x)]
    | .DuplicateGroupId x => variant "DuplicateGroupId" [("group_id", toJson x)]
    | .DuplicatePromotionId x => variant "DuplicatePromotionId" [("promotion_id", toJson x)]
    | .UnknownGroup i => variant "UnknownGroup" [("promotion_index", toJson i)]
    | .Overflow => variant "Overflow"

-- Result -------------------------------------------------------------------

instance : ToJson ClampReason where
  toJson
    | .Cap => variant "Cap"
    | .Budget => variant "Budget"
    | .OrderCap => variant "OrderCap"
    | .Running => variant "Running"

instance : ToJson SkipReason where
  toJson
    | .NotEligible k => variant "NotEligible" [("rule_index", toJson k)]
    | .ExclusiveApplied w => variant "ExclusiveApplied" [("winner", toJson w)]
    | .FirstMatchTaken w => variant "FirstMatchTaken" [("winner", toJson w)]

instance : ToJson SkippedPromotion where
  toJson s := Json.mkObj [("promotion_id", toJson s.promotion_id), ("reason", toJson s.reason)]

instance : ToJson LineAllocation where
  toJson a := Json.mkObj [("line_id", toJson a.line_id), ("amount", toJson a.amount)]

instance : ToJson AppliedPromotion where
  toJson a := Json.mkObj [("promotion_id", toJson a.promotion_id), ("discount", toJson a.discount),
    ("clamped_by", optJson a.clamped_by), ("allocation", toJson a.allocation)]

instance : ToJson LineResult where
  toJson l := Json.mkObj [("line_id", toJson l.line_id), ("subtotal", toJson l.subtotal),
    ("discount", toJson l.discount), ("total", toJson l.total)]

instance : ToJson PricingResult where
  toJson r := Json.mkObj [("currency", toJson r.currency),
    ("original_subtotal", toJson r.original_subtotal), ("total_discount", toJson r.total_discount),
    ("final_total", toJson r.final_total), ("lines", toJson r.lines),
    ("applied", toJson r.applied), ("skipped", toJson r.skipped)]

/-- `{"ok": result}` or `{"error": error}` (the CLI envelope). -/
def envelope : Except PricingError PricingResult → Json
  | .ok r => Json.mkObj [("ok", toJson r)]
  | .error e => Json.mkObj [("error", toJson e)]

end Discount
