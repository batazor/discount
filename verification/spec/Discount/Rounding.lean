import Discount.Types

/-!
# roundDiv (ADR 3)

`q = num / den` (Euclidean division, which is floor for the non-negative `num` used
everywhere in the engine) and `r = num % den`.
-/
namespace Discount

def roundDiv (p : RoundingPolicy) (num den : Int) : Int :=
  let q := num / den
  let r := num % den
  match p with
  | .Down => q
  | .HalfUp => if 2 * r ≥ den then q + 1 else q
  | .HalfEven =>
      if 2 * r > den then q + 1
      else if 2 * r < den then q
      else if q % 2 = 0 then q else q + 1

end Discount
