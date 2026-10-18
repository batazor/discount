import Discount.Json.Input

/-!
# JSON encoders of errors and results, and the CLI envelope
-/
namespace Discount

open Lean (Json ToJson FromJson toJson fromJson?)

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
    | .DuplicateCouponCode c => variant "DuplicateCouponCode" [("code", toJson c)]
    | .InvalidCustomerContext f => variant "InvalidCustomerContext" [("field", toJson f)]
    | .InvalidExchangeRate f => variant "InvalidExchangeRate" [("field", toJson f)]
    | .DuplicateExchangeRate f t => variant "DuplicateExchangeRate" [("from", toJson f), ("to", toJson t)]
    | .InvalidRule f => variant "InvalidRule" [("field", toJson f)]
    | .InvalidCoupon f => variant "InvalidCoupon" [("field", toJson f)]
    | .InvalidPromotion f => variant "InvalidPromotion" [("field", toJson f)]
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
