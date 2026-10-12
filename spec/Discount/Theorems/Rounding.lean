import Discount.Rounding

/-!
# roundDiv lemmas (ADR 3)
-/
namespace Discount

theorem roundDiv_nonneg (p : RoundingPolicy) (num den : Int) (hn : 0 ≤ num) (hd : 0 < den) :
    0 ≤ roundDiv p num den := by
  have hq : 0 ≤ num / den := Int.ediv_nonneg hn (Int.le_of_lt hd)
  unfold roundDiv
  cases p <;> dsimp only <;> (repeat' split) <;> omega

/-- A percentage of at most 100 % never exceeds its base, whatever the policy. -/
theorem roundDiv_le (p : RoundingPolicy) (base bps : Int) (hb : 0 ≤ base) (_h0 : 0 ≤ bps)
    (h1 : bps ≤ BPS_SCALE) : roundDiv p (base * bps) BPS_SCALE ≤ base := by
  have hden : (0 : Int) < BPS_SCALE := by decide
  have hq : (base * bps) / BPS_SCALE ≤ base := by
    calc (base * bps) / BPS_SCALE ≤ (base * BPS_SCALE) / BPS_SCALE :=
          Int.ediv_le_ediv hden (Int.mul_le_mul_of_nonneg_left h1 hb)
      _ = base := Int.mul_ediv_cancel _ (Int.ne_of_gt hden)
  have hr0 : 0 ≤ (base * bps) % BPS_SCALE := Int.emod_nonneg _ (Int.ne_of_gt hden)
  -- if the remainder is positive, the quotient is strictly below `base`
  have hlt : 0 < (base * bps) % BPS_SCALE → (base * bps) / BPS_SCALE < base := by
    intro hr
    have hsplit : BPS_SCALE * ((base * bps) / BPS_SCALE) + (base * bps) % BPS_SCALE = base * bps :=
      Int.mul_ediv_add_emod _ _
    have hle : base * bps ≤ base * BPS_SCALE := Int.mul_le_mul_of_nonneg_left h1 hb
    have : BPS_SCALE * ((base * bps) / BPS_SCALE) < BPS_SCALE * base := by
      rw [Int.mul_comm base BPS_SCALE] at hle; omega
    exact Int.lt_of_mul_lt_mul_left this (Int.le_of_lt hden)
  unfold roundDiv
  cases p <;> dsimp only <;> (repeat' split) <;> first | omega | (have := hlt (by omega); omega)

end Discount
