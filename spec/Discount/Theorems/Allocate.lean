import Discount.Allocate
import Discount.Theorems.Lists

/-!
# Allocation lemmas (ADR 5, invariants 1 and 5)

Under `0 ≤ D ≤ Σ s` and `s_i ≥ 0`:
`allocate_length`, `allocate_sum` (conservation), `allocate_nonneg`, `allocate_le`
(per-line bound).
-/
namespace Discount

/-! ## positiveIndices -/

theorem positiveIndices_length (rs : List Int) (k : Nat) :
    (positiveIndices rs k).length = rs.countP (fun r => decide (0 < r)) := by
  induction rs generalizing k with
  | nil => simp [positiveIndices]
  | cons r rs ih =>
    simp only [positiveIndices, List.countP_cons]
    split <;> simp_all

theorem mem_positiveIndices (rs : List Int) (k i : Nat) (h : i ∈ positiveIndices rs k) :
    k ≤ i ∧ i - k < rs.length ∧ 0 < rs.getD (i - k) 0 := by
  induction rs generalizing k with
  | nil => simp [positiveIndices] at h
  | cons r rs ih =>
    simp only [positiveIndices] at h
    split at h
    · rcases List.mem_cons.mp h with rfl | h'
      · simp_all
      · obtain ⟨h1, h2, h3⟩ := ih (k + 1) h'
        refine ⟨by omega, by simp; omega, ?_⟩
        have : i - k = (i - (k + 1)) + 1 := by omega
        rw [this, List.getD_cons_succ]; exact h3
    · obtain ⟨h1, h2, h3⟩ := ih (k + 1) h
      refine ⟨by omega, by simp; omega, ?_⟩
      have : i - k = (i - (k + 1)) + 1 := by omega
      rw [this, List.getD_cons_succ]; exact h3

theorem mem_positiveIndices_zero (rs : List Int) (i : Nat) (h : i ∈ positiveIndices rs 0) :
    i < rs.length ∧ 0 < rs.getD i 0 := by
  have := mem_positiveIndices rs 0 i h
  simpa using this

theorem positiveIndices_nodup (rs : List Int) (k : Nat) : (positiveIndices rs k).Nodup := by
  induction rs generalizing k with
  | nil => simp [positiveIndices]
  | cons r rs ih =>
    simp only [positiveIndices]
    split
    · rw [List.nodup_cons]
      refine ⟨fun h => ?_, ih (k + 1)⟩
      have := (mem_positiveIndices rs (k + 1) k h).1
      omega
    · exact ih (k + 1)

/-! ## bumpAll -/

theorem bumpAll_nil (base : List Int) : bumpAll base [] = base := rfl

theorem bumpAll_cons (base : List Int) (j : Nat) (ps : List Nat) :
    bumpAll base (j :: ps) = bumpAll (base.set j (base.getD j 0 + 1)) ps := rfl

theorem bumpAll_length (base : List Int) (picked : List Nat) :
    (bumpAll base picked).length = base.length := by
  induction picked generalizing base with
  | nil => rfl
  | cons j ps ih => rw [bumpAll_cons, ih, List.length_set]

theorem bumpAll_sum (base : List Int) (picked : List Nat) (h : ∀ j ∈ picked, j < base.length) :
    (bumpAll base picked).sum = base.sum + picked.length := by
  induction picked generalizing base with
  | nil => simp [bumpAll_nil]
  | cons j ps ih =>
    rw [bumpAll_cons, ih _ (fun x hx => by rw [List.length_set]; exact h x (by simp [hx]))]
    rw [sum_set_succ base j (h j (by simp))]
    simp only [List.length_cons]
    omega

theorem bumpAll_getD (base : List Int) (picked : List Nat) (i : Nat) (hi : i < base.length) :
    (bumpAll base picked).getD i 0 = base.getD i 0 + picked.count i := by
  induction picked generalizing base with
  | nil => simp [bumpAll_nil]
  | cons j ps ih =>
    rw [bumpAll_cons, ih _ (by rw [List.length_set]; exact hi), getD_set_succ, List.count_cons]
    by_cases hji : j = i
    · subst hji; simp [hi]; omega
    · simp [hji]

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

/-! ## pickedLines -/

theorem pickedLines_mem (j : Nat) (hj : j ∈ pickedLines D s.sum s) :
    j < s.length ∧ 0 < (remainders D s.sum s).getD j 0 := by
  unfold pickedLines at hj
  have h1 := (List.take_sublist _ _).subset hj
  rw [List.mem_mergeSort] at h1
  have := mem_positiveIndices_zero _ _ h1
  rwa [remainders_length] at this

theorem pickedLines_nodup : (pickedLines D s.sum s).Nodup := by
  unfold pickedLines
  exact List.Nodup.sublist (List.take_sublist _ _)
    ((List.mergeSort_perm _ _).nodup_iff.mpr (positiveIndices_nodup _ _))

theorem pickedLines_length (hS : 0 < s.sum) : ((pickedLines D s.sum s).length : Int) = extraUnits D s.sum s := by
  have hR0 := extraUnits_nonneg D s hS
  have hRc := extraUnits_le_candidates D s hS
  unfold pickedLines
  rw [List.length_take, List.length_mergeSort]
  have ht : ((extraUnits D s.sum s).toNat : Int) = extraUnits D s.sum s := Int.toNat_of_nonneg hR0
  have hmin : min (extraUnits D s.sum s).toNat (positiveIndices (remainders D s.sum s) 0).length
      = (extraUnits D s.sum s).toNat := Nat.min_eq_left (by omega)
  rw [hmin, ht]

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
