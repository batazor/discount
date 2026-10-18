

/-!
# Money, identifiers and bounds (ADR 3)

Names follow `docs/ubiquitous-language.md` exactly (ADR 8). Constructor names are the JSON
variant names, field names the JSON keys; the same holds for every file under `Types/`.
-/
namespace Discount

abbrev Currency := String
abbrev LineId := String
abbrev GroupId := String
abbrev PromotionId := String
abbrev BasisPoints := Int
abbrev Timestamp := Int
abbrev Quantity := Int

/-- Upper bound for every amount and for the order subtotal (ADR 3). -/
def MAX_MINOR_UNITS : Int := 100000000000000
/-- Upper bound for `quantity`. -/
def MAX_QUANTITY : Int := 1000000000
/-- Denominator of `BasisPoints`. -/
def BPS_SCALE : Int := 10000
/-- Upper bound for `ExchangeRate.numerator` and `denominator`. -/
def MAX_RATE_PART : Int := 1000000000000

structure Money where
  amount : Int
  currency : Currency
  deriving Repr, DecidableEq, Inhabited

inductive RoundingPolicy where
  | HalfUp
  | HalfEven
  | Down
  deriving Repr, DecidableEq, Inhabited

end Discount
