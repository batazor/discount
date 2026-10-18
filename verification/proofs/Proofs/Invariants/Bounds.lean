import Proofs.Compute

/-!
# Invariant 1: the final price is within bounds (`docs/invariants.md`)
-/
namespace Discount

/-! ## 1. Final price is within bounds -/

theorem final_total_bounds {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    0 ≤ r.final_total.amount ∧ r.final_total.amount ≤ r.original_subtotal.amount ∧
      r.final_total.amount = r.original_subtotal.amount - r.total_discount.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  rw [compute_final_total, compute_original, compute_total_discount]
  have h1 := lineRunning_sum_nonneg hv
  have h2 := lineRunning_sum_le hv
  omega

theorem line_total_bounds {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ l ∈ r.lines, 0 ≤ l.total.amount ∧ l.total.amount ≤ l.subtotal.amount ∧
      l.total.amount = l.subtotal.amount - l.discount.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro l hl
  rw [compute_lines] at hl
  obtain ⟨k, hk, rfl⟩ := List.mem_iff_getElem.mp hl
  have hk' : k < (subsOf (normalize i)).length := by
    simp only [List.length_zipWith, List.length_zip] at hk; omega
  have hb := hinv.bounds k hk'
  rw [getD_eq_getElem _ _ (hinv.len ▸ hk'), getD_eq_getElem _ _ hk'] at hb
  simp only [List.getElem_zipWith, List.getElem_zip]
  omega

end Discount
