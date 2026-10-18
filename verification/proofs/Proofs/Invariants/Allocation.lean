import Proofs.Compute

/-!
# Invariant 5: allocation is conservative
-/
namespace Discount

/-! ## 5. Allocation is conservative -/

theorem allocation_conserves {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied,
      (a.allocation.map (fun x => x.amount.amount)).sum = a.discount.amount ∧
      a.allocation.length = r.lines.length ∧
      a.allocation.map (fun x => x.line_id) = (normalize i).cart.lines.map (fun l => l.line_id) := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha
  rw [compute_applied] at ha
  have hok := hinv.applied_ok a ha
  refine ⟨hok.alloc_sum, ?_, hok.alloc_ids⟩
  rw [compute_lines_length hv, hok.alloc_len]

theorem zipLines_discount_getD (cur : Currency) (lines : List CartLine) (subs lr : List Int) (k : Nat)
    (h1 : lines.length = subs.length) (h2 : subs.length = lr.length) (hk : k < lines.length) :
    ((List.zipWith (fun (l : CartLine) (sl : Int × Int) =>
      ({ line_id := l.line_id, subtotal := ⟨sl.1, cur⟩, discount := ⟨sl.1 - sl.2, cur⟩,
         total := ⟨sl.2, cur⟩ } : LineResult)) lines (subs.zip lr)).map
        (fun l => l.discount.amount)).getD k 0 = subs.getD k 0 - lr.getD k 0 := by
  induction lines generalizing subs lr k with
  | nil => simp at hk
  | cons l ls ih =>
    cases subs with
    | nil => simp at h1
    | cons s ss =>
      cases lr with
      | nil => simp at h2
      | cons x xs =>
        cases k with
        | zero => simp
        | succ k =>
          simp only [List.zip_cons_cons, List.zipWith_cons_cons, List.map_cons, List.getD_cons_succ]
          exact ih ss xs k (by simpa using h1) (by simpa using h2) (by simpa using hk)

theorem line_discount_sum {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ k, k < r.lines.length →
      (r.lines.map (fun l => l.discount.amount)).getD k 0 =
        (r.applied.map (fun a => (a.allocation.map (fun x => x.amount.amount)).getD k 0)).sum := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro k hk
  have hk' : k < (subsOf (normalize i)).length := by rw [← compute_lines_length hv]; exact hk
  rw [compute_applied, hinv.line_disc k hk', compute_lines]
  exact zipLines_discount_getD _ _ _ _ k (subsOf_length (normalize i)).symm hinv.len.symm
    (by rw [← subsOf_length]; exact hk')

end Discount
