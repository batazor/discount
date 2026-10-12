import Discount.Types

/-!
# allocate (ADR 5)

Largest-remainder allocation of an order-level discount `D` over line subtotals `s`
(`S = s.sum`). Only lines with a positive remainder are candidates for an extra unit; this
is a provable no-op with respect to "the `R` lines with the largest remainders" (a line
with remainder zero can never be among them when `R > 0`) and it makes the per-line bound
immediate. The pieces are named so the theorems can speak about them.
-/
namespace Discount

/-- Indices of the positive entries, in order, starting at offset `k`. -/
def positiveIndices : List Int → Nat → List Nat
  | [], _ => []
  | r :: rs, k => if 0 < r then k :: positiveIndices rs (k + 1) else positiveIndices rs (k + 1)

/-- `r_j < r_i`, or equal remainders and `i` comes first: largest remainder, ties by position. -/
def remainderLe (rems : List Int) (i j : Nat) : Bool :=
  decide (rems.getD j 0 < rems.getD i 0) || (rems.getD i 0 == rems.getD j 0 && decide (i ≤ j))

/-- Add one minor unit to every picked line (a line picked twice would get two; `pickedLines`
has no duplicates). -/
def bumpAll (base : List Int) (picked : List Nat) : List Int :=
  picked.foldl (fun acc j => acc.set j (acc.getD j 0 + 1)) base

/-- `floor(D * s_i / S)` per line. -/
def baseShares (D S : Int) (s : List Int) : List Int := s.map (fun si => (D * si) / S)

/-- `(D * s_i) mod S` per line. -/
def remainders (D S : Int) (s : List Int) : List Int := s.map (fun si => (D * si) % S)

/-- `R = D - Σ base_i`, the minor units still to distribute. -/
def extraUnits (D S : Int) (s : List Int) : Int := D - (baseShares D S s).sum

/-- The `R` candidate lines with the largest remainders, ties by position. -/
def pickedLines (D S : Int) (s : List Int) : List Nat :=
  let rems := remainders D S s
  ((positiveIndices rems 0).mergeSort (remainderLe rems)).take (extraUnits D S s).toNat

def allocate (D : Int) (s : List Int) : List Int :=
  let S := s.sum
  if S ≤ 0 then s.map (fun _ => 0)
  else bumpAll (baseShares D S s) (pickedLines D S s)

end Discount
