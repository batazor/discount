import Discount.Allocate
import Proofs.Lists

/-!
# Allocation lemmas, part 1: candidate indices and `bumpAll` (ADR 5)
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

end Discount
