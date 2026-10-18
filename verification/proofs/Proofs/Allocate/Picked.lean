import Proofs.Allocate.Shares

/-!
# Allocation lemmas, part 3: the `R` lines picked for an extra unit (ADR 5)
-/
namespace Discount

section Shares

variable (D : Int) (s : List Int)

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
end Shares

end Discount
