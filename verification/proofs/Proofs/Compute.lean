import Proofs.Fold

/-!
# Plumbing for the invariant theorems

`apply_ok` opens `apply input = .ok result`; the `compute_*` lemmas give the shape of each
result field; the nodup lemmas identify a promotion by its id.
-/
namespace Discount

theorem apply_ok {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    validate i = .ok () ∧ r = compute (normalize i) := by
  unfold apply at h
  split at h
  · exact nomatch h
  · exact ⟨by assumption, (Except.ok.inj h).symm⟩

theorem getD_eq_getElem (l : List Int) (k : Nat) (hk : k < l.length) : l.getD k 0 = l[k] := by
  rw [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem hk]; rfl

theorem find?_eq_some_of_nodup {l : List Promotion} (h : (l.map (fun p => p.promotion_id)).Nodup)
    {q : Promotion} (hq : q ∈ l) :
    l.find? (fun p => p.promotion_id == q.promotion_id) = some q := by
  induction l with
  | nil => simp at hq
  | cons x xs ih =>
    by_cases hx : x.promotion_id = q.promotion_id
    · rw [List.find?_cons_of_pos (by simp [hx])]
      have : x = q := eq_of_nodup_map h (by simp) hq hx
      rw [this]
    · rw [List.find?_cons_of_neg (by simp [hx])]
      have hq' : q ∈ xs := by
        rcases List.mem_cons.mp hq with rfl | hq'
        · exact absurd rfl hx
        · exact hq'
      exact ih (List.nodup_cons.mp (by simpa [List.map_cons] using h)).2 hq'

/-- Shape of `compute`'s fields. -/
theorem compute_original (i : PricingInput) : (compute i).original_subtotal.amount = (subsOf i).sum := rfl
theorem compute_total_discount (i : PricingInput) :
    (compute i).total_discount.amount = (subsOf i).sum - (finalStep i).lineRunning.sum := rfl
theorem compute_final_total (i : PricingInput) :
    (compute i).final_total.amount = (subsOf i).sum - ((subsOf i).sum - (finalStep i).lineRunning.sum) := rfl
theorem compute_applied (i : PricingInput) : (compute i).applied = (finalStep i).applied := rfl
theorem compute_lines (i : PricingInput) :
    (compute i).lines = List.zipWith (fun (l : CartLine) (sl : Int × Int) =>
      ({ line_id := l.line_id, subtotal := ⟨sl.1, i.cart.currency⟩,
         discount := ⟨sl.1 - sl.2, i.cart.currency⟩, total := ⟨sl.2, i.cart.currency⟩ } : LineResult))
      i.cart.lines ((subsOf i).zip (finalStep i).lineRunning) := rfl

theorem compute_lines_length {i : PricingInput} (hv : Valid i) :
    (compute i).lines.length = (subsOf i).length := by
  have hinv := finalStep_inv hv
  rw [compute_lines, List.length_zipWith, List.length_zip, hinv.len, subsOf_length, Nat.min_self,
    Nat.min_self]

theorem lineRunning_sum_nonneg {i : PricingInput} (hv : Valid i) : 0 ≤ (finalStep i).lineRunning.sum :=
  sum_nonneg_of_forall _ (lineRunning_nonneg (finalStep_inv hv))

theorem lineRunning_sum_le {i : PricingInput} (hv : Valid i) :
    (finalStep i).lineRunning.sum ≤ (subsOf i).sum := by
  have hinv := finalStep_inv hv
  exact sum_le_sum_of_getD _ _ hinv.len (fun k hk => (hinv.bounds k (hinv.len ▸ hk)).2)

end Discount
