import Proofs.Allocate.Indices

/-!
# Allocation lemmas, part 2: arithmetic of `baseShares`, `remainders`, `extraUnits` (ADR 5)

Under `0 < Σ s`, `s_i ≥ 0`, `0 ≤ D ≤ Σ s`.
-/
namespace Discount

/-! ## Arithmetic of the shares -/

section Shares

variable (D : Int) (s : List Int)

theorem baseShares_length : (baseShares D s.sum s).length = s.length := by
  simp [baseShares]

theorem remainders_length : (remainders D s.sum s).length = s.length := by
  simp [remainders]

theorem baseShares_getD (i : Nat) (hi : i < s.length) :
    (baseShares D s.sum s).getD i 0 = (D * s.getD i 0) / s.sum :=
  getD_map_lt s _ i hi

theorem remainders_getD (i : Nat) (hi : i < s.length) :
    (remainders D s.sum s).getD i 0 = (D * s.getD i 0) % s.sum :=
  getD_map_lt s _ i hi

theorem baseShares_nonneg (hS : 0 < s.sum) (hs : ∀ x ∈ s, 0 ≤ x) (hD0 : 0 ≤ D) (i : Nat) (hi : i < s.length) : 0 ≤ (baseShares D s.sum s).getD i 0 := by
  rw [baseShares_getD D s i hi]
  exact Int.ediv_nonneg (Int.mul_nonneg hD0 (hs _ (getD_mem_of_lt s i hi))) (Int.le_of_lt hS)

theorem baseShares_le (hS : 0 < s.sum) (hs : ∀ x ∈ s, 0 ≤ x) (hDS : D ≤ s.sum) (i : Nat) (hi : i < s.length) :
    (baseShares D s.sum s).getD i 0 ≤ s.getD i 0 := by
  rw [baseShares_getD D s i hi]
  have hsi : 0 ≤ s.getD i 0 := hs _ (getD_mem_of_lt s i hi)
  calc (D * s.getD i 0) / s.sum ≤ (s.sum * s.getD i 0) / s.sum :=
        Int.ediv_le_ediv hS (Int.mul_le_mul_of_nonneg_right hDS hsi)
    _ = s.getD i 0 := Int.mul_ediv_cancel_left _ (Int.ne_of_gt hS)

/-- Strict version for lines with a positive remainder. -/
theorem baseShares_lt (hS : 0 < s.sum) (hs : ∀ x ∈ s, 0 ≤ x) (hDS : D ≤ s.sum) (i : Nat) (hi : i < s.length) (hr : 0 < (remainders D s.sum s).getD i 0) :
    (baseShares D s.sum s).getD i 0 < s.getD i 0 := by
  rw [baseShares_getD D s i hi]
  rw [remainders_getD D s i hi] at hr
  have hsi : 0 ≤ s.getD i 0 := hs _ (getD_mem_of_lt s i hi)
  have h1 : s.sum * (D * s.getD i 0 / s.sum) + (D * s.getD i 0) % s.sum = D * s.getD i 0 :=
    Int.mul_ediv_add_emod _ _
  have h2 : D * s.getD i 0 ≤ s.sum * s.getD i 0 := Int.mul_le_mul_of_nonneg_right hDS hsi
  have h3 : s.sum * (D * s.getD i 0 / s.sum) < s.sum * s.getD i 0 := by omega
  exact Int.lt_of_mul_lt_mul_left h3 (Int.le_of_lt hS)

theorem baseShares_sum_le (hS : 0 < s.sum) : (baseShares D s.sum s).sum ≤ D := by
  have h1 : s.sum * (baseShares D s.sum s).sum = (s.map (fun si => s.sum * ((D * si) / s.sum))).sum := by
    rw [baseShares, sum_map_mul_left]
  have h2 : (s.map (fun si => s.sum * ((D * si) / s.sum))).sum ≤ (s.map (fun si => D * si)).sum :=
    sum_map_le_sum_map s _ _ (fun x _ => Int.mul_ediv_self_le (Int.ne_of_gt hS))
  have h3 : (s.map (fun si => D * si)).sum = D * s.sum := by
    have := sum_map_mul_left D s id
    simpa using this
  have h4 : s.sum * (baseShares D s.sum s).sum ≤ s.sum * D := by
    rw [h1, Int.mul_comm s.sum D]; exact Int.le_trans h2 (Int.le_of_eq h3)
  exact Int.le_of_mul_le_mul_left h4 hS

theorem extraUnits_nonneg (hS : 0 < s.sum) : 0 ≤ extraUnits D s.sum s := by
  have := baseShares_sum_le D s hS
  unfold extraUnits; omega

theorem remainders_sum : (remainders D s.sum s).sum = s.sum * extraUnits D s.sum s := by
  have h1 : remainders D s.sum s = s.map (fun si => D * si - s.sum * ((D * si) / s.sum)) := by
    unfold remainders
    exact List.map_congr_left (fun si _ => Int.emod_def _ _)
  have h2 : (s.map (fun si => D * si)).sum = D * s.sum := by
    have := sum_map_mul_left D s id
    simpa using this
  have h3 : (s.map (fun si => s.sum * ((D * si) / s.sum))).sum = s.sum * (baseShares D s.sum s).sum := by
    rw [baseShares, sum_map_mul_left]
  rw [h1, sum_map_sub, h2, h3]
  unfold extraUnits
  rw [Int.mul_sub, Int.mul_comm D s.sum]

theorem mem_remainders_bounds (hS : 0 < s.sum) (x : Int) (hx : x ∈ remainders D s.sum s) : 0 ≤ x ∧ x < s.sum := by
  unfold remainders at hx
  rw [List.mem_map] at hx
  obtain ⟨si, _, rfl⟩ := hx
  exact ⟨Int.emod_nonneg _ (Int.ne_of_gt hS), Int.emod_lt_of_pos _ hS⟩

theorem extraUnits_le_candidates (hS : 0 < s.sum) :
    extraUnits D s.sum s ≤ ((positiveIndices (remainders D s.sum s) 0).length : Int) := by
  have hR := remainders_sum D s
  have hc' : ((positiveIndices (remainders D s.sum s) 0).length : Int)
      = ((remainders D s.sum s).countP (fun r => decide (0 < r)) : Int) := by
    rw [positiveIndices_length]
  have hle : (remainders D s.sum s).sum ≤
      ((remainders D s.sum s).map (fun r => if decide (0 < r) then s.sum - 1 else 0)).sum := by
    have := sum_map_le_sum_map (remainders D s.sum s) id
      (fun r => if decide (0 < r) then s.sum - 1 else 0)
      (fun x hx => by
        have hb := mem_remainders_bounds D s hS x hx
        simp only [id]
        split <;> rename_i hd <;> simp at hd <;> omega)
    simpa using this
  rw [sum_map_ite, hR, ← hc'] at hle
  rw [Int.sub_mul, Int.one_mul] at hle
  generalize extraUnits D s.sum s = R at *
  generalize (positiveIndices (remainders D s.sum s) 0).length = c at *
  by_cases hfin : R ≤ (c : Int)
  · exact hfin
  · have hlt : (c : Int) + 1 ≤ R := by omega
    have h1 : s.sum * ((c : Int) + 1) ≤ s.sum * R :=
      Int.mul_le_mul_of_nonneg_left hlt (Int.le_of_lt hS)
    rw [Int.mul_add, Int.mul_one] at h1
    omega
end Shares

end Discount
