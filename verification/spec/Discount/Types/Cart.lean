import Discount.Types.Money

/-!
# Cart snapshot

`CartLine`, `Attribute`, `CartSnapshot` and the two subtotals the engine starts from.
-/
namespace Discount

structure CartLine where
  line_id : LineId
  unit_price : Money
  quantity : Quantity
  deriving Repr, DecidableEq, Inhabited

/-- A string key and value describing the order. -/
structure Attribute where
  key : String
  value : String
  deriving Repr, DecidableEq, Inhabited

structure CartSnapshot where
  currency : Currency
  lines : List CartLine
  tags : List String
  attributes : List Attribute
  coupon_codes : List String
  deriving Repr, DecidableEq, Inhabited

/-- `unit_price.amount * quantity` (ADR 8). -/
def lineSubtotal (l : CartLine) : Int := l.unit_price.amount * l.quantity

/-- Sum of line subtotals. -/
def orderSubtotal (c : CartSnapshot) : Int := (c.lines.map lineSubtotal).sum

end Discount
