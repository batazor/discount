import Discount.Rounding

/-!
# `roundDiv` lemmas (ADR 3)

`roundDiv p num den` is the only place the engine turns a fraction into minor units:

```
q := num / den          -- Euclidean quotient; for num ≥ 0, den > 0 this is floor(num/den)
r := num % den          -- Euclidean remainder; 0 ≤ r < den
Down     ↦ q
HalfUp   ↦ if 2r ≥ den then q + 1 else q           -- ".5 and above rounds up"
HalfEven ↦ if 2r > den then q + 1                   -- above .5: up
           else if 2r < den then q                  -- below .5: down
           else (if q even then q else q + 1)       -- exactly .5: to the even neighbour
```

Every policy returns either `q` or `q + 1`. That single observation drives both lemmas:
whatever we can say about `q` and `q + 1` holds for the result, so each proof establishes
a fact about `q`, then lets Lean walk the `if`/`match` tree and finish each leaf with
`omega` (linear integer arithmetic).

Two lemmas are proved:

* `roundDiv_nonneg`: a rounded non-negative fraction is non-negative. Used by the fold
  invariant (`Fold.lean`) to show that a raw percentage discount is `≥ 0` before clamping,
  and by `orderCapInitial_nonneg` for the percentage part of the order cap.
* `roundDiv_le`: `roundDiv p (base * bps) BPS_SCALE ≤ base` for `0 ≤ bps ≤ BPS_SCALE`,
  i.e. a discount of at most 100 % never exceeds what it is taken from, even with `HalfUp`.
  The pipeline does not need it (the clamp to the running subtotal already bounds every
  discount), but it is the statement a reader expects, and it documents why rounding up
  cannot overshoot: when `bps = 10 000` the remainder is zero, so nothing rounds up.

Library facts used (all from Lean core, no Mathlib):

* `Int.ediv_nonneg : 0 ≤ a → 0 ≤ b → 0 ≤ a / b`
* `Int.emod_nonneg : b ≠ 0 → 0 ≤ a % b`
* `Int.mul_ediv_add_emod : b * (a / b) + a % b = a`   (the division identity)
* `Int.ediv_le_ediv : 0 < c → a ≤ b → a / c ≤ b / c`   (division is monotone)
* `Int.mul_ediv_cancel : b ≠ 0 → a * b / b = a`
* `Int.mul_le_mul_of_nonneg_left : a ≤ b → 0 ≤ c → c * a ≤ c * b`
* `Int.lt_of_mul_lt_mul_left : a * b < a * c → 0 ≤ a → b < c`   (cancel a positive factor)
-/
namespace Discount

/-- **Non-negativity.** For `0 ≤ num` and `0 < den`, `roundDiv p num den ≥ 0` for every
policy.

Proof idea: the quotient `q = num / den` is non-negative (`Int.ediv_nonneg`), and every
policy returns `q` or `q + 1`. After unfolding the definition and splitting on the policy
and on each `if`, every leaf is a linear fact about `q`, which `omega` closes using `hq`. -/
theorem roundDiv_nonneg (p : RoundingPolicy) (num den : Int) (hn : 0 ≤ num) (hd : 0 < den) :
    0 ≤ roundDiv p num den := by
  -- The one arithmetic fact: a non-negative number divided by a positive one is non-negative.
  have hq : 0 ≤ num / den := Int.ediv_nonneg hn (Int.le_of_lt hd)
  -- Expose `q`, `r` and the `match`/`if` structure.
  unfold roundDiv
  -- `cases p` makes three goals (Down, HalfUp, HalfEven); `dsimp only` reduces the `match`
  -- for the concrete constructor; `repeat' split` turns every remaining `if` into two goals.
  -- Each leaf is `0 ≤ q` or `0 ≤ q + 1`, with `hq` in context: linear arithmetic.
  cases p <;> dsimp only <;> (repeat' split) <;> omega

/-- **A percentage never exceeds its base.** For `0 ≤ base` and `0 ≤ bps ≤ BPS_SCALE`,
`roundDiv p (base * bps) BPS_SCALE ≤ base` for every policy.

Proof idea, writing `S = BPS_SCALE = 10 000`, `q = (base * bps) / S`, `r = (base * bps) % S`:

1. `q ≤ base`: because `base * bps ≤ base * S` (monotone in `bps`) and division by `S` is
   monotone, `q ≤ (base * S) / S = base`. This alone settles every leaf that returns `q`.
2. Leaves that return `q + 1` need `q < base`, i.e. `q + 1 ≤ base`. They are reached only
   when the remainder is positive: `HalfUp` needs `2r ≥ S`, `HalfEven` needs `2r > S` or
   `2r = S`; with `0 ≤ r`, each of these forces `r > 0` (because `S > 0`).
3. `r > 0 → q < base`: from the division identity `S * q + r = base * bps` and
   `base * bps ≤ base * S` we get `S * q < S * base`, and cancelling the positive factor `S`
   gives `q < base`.

The tactic script establishes (1) as `hq`, (3) as `hlt`, then unfolds and splits as in
`roundDiv_nonneg`; each leaf is closed either by `omega` directly (returns `q`, or a
contradictory branch condition) or by first obtaining `q < base` from `hlt` with the branch
condition (`by omega` shows `0 < r` from e.g. `2r ≥ S`), then `omega` again. -/
theorem roundDiv_le (p : RoundingPolicy) (base bps : Int) (hb : 0 ≤ base) (_h0 : 0 ≤ bps)
    (h1 : bps ≤ BPS_SCALE) : roundDiv p (base * bps) BPS_SCALE ≤ base := by
  -- `BPS_SCALE` is a closed term (10 000); `decide` evaluates the comparison.
  have hden : (0 : Int) < BPS_SCALE := by decide
  -- (1) The quotient is at most `base`.
  have hq : (base * bps) / BPS_SCALE ≤ base := by
    calc (base * bps) / BPS_SCALE ≤ (base * BPS_SCALE) / BPS_SCALE :=
          Int.ediv_le_ediv hden (Int.mul_le_mul_of_nonneg_left h1 hb)
      _ = base := Int.mul_ediv_cancel _ (Int.ne_of_gt hden)
  -- Remainders are never negative.
  have hr0 : 0 ≤ (base * bps) % BPS_SCALE := Int.emod_nonneg _ (Int.ne_of_gt hden)
  -- (3) A positive remainder means the quotient is strictly below `base`.
  have hlt : 0 < (base * bps) % BPS_SCALE → (base * bps) / BPS_SCALE < base := by
    intro hr
    -- Division identity: S * q + r = base * bps.
    have hsplit : BPS_SCALE * ((base * bps) / BPS_SCALE) + (base * bps) % BPS_SCALE = base * bps :=
      Int.mul_ediv_add_emod _ _
    -- Upper bound on the numerator: base * bps ≤ base * S.
    have hle : base * bps ≤ base * BPS_SCALE := Int.mul_le_mul_of_nonneg_left h1 hb
    -- Combine: S * q < S * q + r = base * bps ≤ base * S = S * base.
    have : BPS_SCALE * ((base * bps) / BPS_SCALE) < BPS_SCALE * base := by
      rw [Int.mul_comm base BPS_SCALE] at hle; omega
    -- Cancel the positive factor S.
    exact Int.lt_of_mul_lt_mul_left this (Int.le_of_lt hden)
  -- (2) Walk the definition. Leaves returning `q` close with `hq`; leaves returning `q + 1`
  -- carry a branch condition that implies `0 < r`, so `hlt` gives `q < base`.
  unfold roundDiv
  cases p <;> dsimp only <;> (repeat' split) <;> first | omega | (have := hlt (by omega); omega)

/-- Two-sided rounding error at the basis-point scale, for every policy:
`BPS_SCALE * roundDiv p x BPS_SCALE - x` lies in `(-BPS_SCALE, BPS_SCALE / 2]`. `Down` never
rounds up (error in `(-BPS_SCALE, 0]`); the half policies move by at most half a step. -/
theorem roundDiv_err (p : RoundingPolicy) (x : Int) :
    -BPS_SCALE < BPS_SCALE * roundDiv p x BPS_SCALE - x ∧
      2 * (BPS_SCALE * roundDiv p x BPS_SCALE - x) ≤ BPS_SCALE := by
  -- `omega` knows Euclidean division and remainder by a literal.
  unfold roundDiv BPS_SCALE
  cases p <;> dsimp only
  -- HalfUp
  · split <;> constructor <;> omega
  -- HalfEven
  · split
    · constructor <;> omega
    · split
      · constructor <;> omega
      · split <;> constructor <;> omega
  -- Down
  · constructor <;> omega

end Discount
