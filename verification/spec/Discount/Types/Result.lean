import Discount.Types.Money

/-!
# Errors and the pricing result (ADR 6, ADR 9)
-/
namespace Discount

inductive PricingError where
  | InvalidCurrency (field : String)
  | MixedCurrencies (line_index : Nat)
  | CurrencyMismatch (field : String)
  | AmountOutOfRange (field : String)
  | InvalidBasisPoints (field : String)
  | InvalidQuantity (line_index : Nat)
  | DuplicateLineId (line_id : LineId)
  | DuplicateGroupId (group_id : GroupId)
  | DuplicatePromotionId (promotion_id : PromotionId)
  | UnknownGroup (promotion_index : Nat)
  | DuplicateCouponCode (code : String)
  | InvalidCustomerContext (field : String)
  | InvalidExchangeRate (field : String)
  | DuplicateExchangeRate («from» : Currency) (to : Currency)
  | InvalidRule (field : String)
  | InvalidCoupon (field : String)
  | InvalidPromotion (field : String)
  /-- Rust only; the Lean model never produces it (ADR 3). -/
  | Overflow
  deriving Repr, DecidableEq, Inhabited

inductive ClampReason where
  | Cap
  | Budget
  | OrderCap
  | Running
  deriving Repr, DecidableEq, Inhabited

inductive SkipReason where
  | NotEligible (rule_index : Nat)
  | ExclusiveApplied (winner : PromotionId)
  | FirstMatchTaken (winner : PromotionId)
  deriving Repr, DecidableEq, Inhabited

structure SkippedPromotion where
  promotion_id : PromotionId
  reason : SkipReason
  deriving Repr, DecidableEq, Inhabited

structure LineAllocation where
  line_id : LineId
  amount : Money
  deriving Repr, DecidableEq, Inhabited

structure AppliedPromotion where
  promotion_id : PromotionId
  discount : Money
  clamped_by : Option ClampReason
  allocation : List LineAllocation
  deriving Repr, DecidableEq, Inhabited

structure LineResult where
  line_id : LineId
  subtotal : Money
  discount : Money
  total : Money
  deriving Repr, DecidableEq, Inhabited

structure PricingResult where
  currency : Currency
  original_subtotal : Money
  total_discount : Money
  final_total : Money
  lines : List LineResult
  applied : List AppliedPromotion
  skipped : List SkippedPromotion
  deriving Repr, DecidableEq, Inhabited

end Discount
