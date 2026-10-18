import Proofs.Allocate.Picked

/-!
# Allocation (ADR 5, invariants 1 and 5)

Under `0 ≤ D ≤ Σ s` and `s_i ≥ 0`: `allocate_length`, `allocate_sum` (conservation),
`allocate_nonneg`, `allocate_le` (per-line bound). The pieces are proved in `Allocate/`.
-/
namespace Discount

section Shares

variable (D : Int) (s : List Int)

/-! ## allocate -/

theorem allocate_sum (hs : ∀ x ∈ s, 0 ≤ x) (hD0 : 0 ≤ D) (hDS : D ≤ s.sum) : (allocate D s).sum = D := by
  unfold allocate
  dsimp only
  split
  · have hS0 : 0 ≤ s.sum := sum_nonneg_of_forall s hs
    have : D = 0 := by omega
    rw [this, sum_map_zero]
  · rename_i hpos
    have hS' : 0 < s.sum := by omega
    rw [bumpAll_sum _ _ (fun j hj => by
      rw [baseShares_length]; exact (pickedLines_mem D s j hj).1)]
    rw [pickedLines_length D s hS']
    unfold extraUnits; omega

theorem allocate_nonneg (hs : ∀ x ∈ s, 0 ≤ x) (hD0 : 0 ≤ D) (i : Nat) (hi : i < s.length) :
    0 ≤ (allocate D s).getD i 0 := by
  unfold allocate
  dsimp only
  split
  · rw [getD_map_zero]; exact Int.le_refl _
  · rename_i hpos
    have hS' : 0 < s.sum := by omega
    rw [bumpAll_getD _ _ i (by rw [baseShares_length]; exact hi)]
    have := baseShares_nonneg D s hS' hs hD0 i hi
    omega

theorem allocate_le (hs : ∀ x ∈ s, 0 ≤ x) (hDS : D ≤ s.sum) (i : Nat) (hi : i < s.length) :
    (allocate D s).getD i 0 ≤ s.getD i 0 := by
  unfold allocate
  dsimp only
  split
  · rw [getD_map_zero]; exact hs _ (getD_mem_of_lt s i hi)
  · rename_i hpos
    have hS' : 0 < s.sum := by omega
    rw [bumpAll_getD _ _ i (by rw [baseShares_length]; exact hi)]
    by_cases hmem : i ∈ pickedLines D s.sum s
    · rw [List.Nodup.count_of_mem (pickedLines_nodup D s) hmem]
      have := baseShares_lt D s hS' hs hDS i hi (pickedLines_mem D s i hmem).2
      omega
    · rw [List.count_eq_zero.mpr hmem]
      have := baseShares_le D s hS' hs hDS i hi
      omega

end Shares

theorem allocate_length (D : Int) (s : List Int) : (allocate D s).length = s.length := by
  unfold allocate
  dsimp only
  split
  · simp
  · rw [bumpAll_length, baseShares_length]

end Discount
