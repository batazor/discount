import Proofs.Fold.Invariant
import Proofs.Stacking

/-!
# From the initial step to `finalStep` for a validated input
-/
namespace Discount

/-! ## The initial step -/

theorem orderCapInitial_nonneg {i : PricingInput} (hv : Valid i) (S : Int) (hS : 0 ≤ S) :
    ∀ o, orderCapInitial i.policy S = some o → 0 ≤ o := by
  intro o ho
  unfold orderCapInitial at ho
  split at ho
  · exact nomatch ho
  · rename_i m hm _
    simp at ho; subst ho
    exact (hv.order_cap_amount m hm).2.1
  · rename_i b _ hb
    simp at ho; subst ho
    exact roundDiv_nonneg _ _ _ (Int.mul_nonneg hS (hv.order_cap_bps b hb).1) (by decide)
  · rename_i m b hm hb
    simp at ho; subst ho
    have h1 := (hv.order_cap_amount m hm).2.1
    have h2 := roundDiv_nonneg i.policy.rounding (S * b) BPS_SCALE
      (Int.mul_nonneg hS (hv.order_cap_bps b hb).1) (by decide)
    omega

theorem subsOf_nonneg {i : PricingInput} (hv : Valid i) : ∀ x ∈ subsOf i, 0 ≤ x := by
  intro x hx
  unfold subsOf at hx
  rw [List.mem_map] at hx
  obtain ⟨l, hl, rfl⟩ := hx
  have hlo := hv.lines l hl
  unfold lineSubtotal
  exact Int.mul_nonneg hlo.amount_nonneg (Int.le_trans (by decide) hlo.quantity_pos)

theorem subsOf_length (i : PricingInput) : (subsOf i).length = i.cart.lines.length := by
  simp [subsOf]

theorem initStep_inv {i : PricingInput} (hv : Valid i) (ps : List Promotion) :
    FoldInv i (subsOf i) (subsOf i).sum ps (initStep i) := by
  have hS0 : 0 ≤ (subsOf i).sum := sum_nonneg_of_forall _ (subsOf_nonneg hv)
  refine
    { len := rfl, bounds := ?_, total := ?_, cap_some := ?_, cap_none := ?_, cap_nonneg := ?_,
      applied_ok := ?_, line_disc := ?_ }
  · intro k hk
    exact ⟨(forall_mem_of_forall_getD _ _ (fun k hk => (subsOf_nonneg hv) _ (getD_mem_of_lt _ k hk)))
      |> fun h => h _ (getD_mem_of_lt _ k hk), Int.le_refl _⟩
  · simp [sumDisc, initStep]
  · intro o0 ho0; simp [initStep, sumDisc, ho0]
  · intro hnone; simp [initStep, hnone]
  · exact orderCapInitial_nonneg hv _ hS0
  · intro a ha; simp [initStep] at ha
  · intro k hk; simp [initStep]

/-- What the whole fold guarantees for a validated input. -/
theorem toApply_sub (i : PricingInput) : ∀ q ∈ toApplyOf i, q ∈ i.promotions := by
  intro q hq
  unfold toApplyOf afterFirstMatch at hq
  have h1 := selectFirstMatch_mem hq
  unfold afterExclusive at h1
  have h2 := selectExclusive_mem h1
  unfold eligibleOf sortedOf sortPromotions at h2
  exact List.mem_mergeSort.mp (List.mem_filter.mp h2).1

theorem finalStep_inv {i : PricingInput} (hv : Valid i) :
    FoldInv i (subsOf i) (subsOf i).sum (toApplyOf i) (finalStep i) :=
  foldl_inv (subsOf_nonneg hv) rfl (subsOf_length i)
    (fun p hp => hv.promotions p (toApply_sub i p hp)) (toApplyOf i) (initStep i)
    (fun _ h => h) (initStep_inv hv (toApplyOf i))

theorem finalStep_ids (i : PricingInput) :
    (finalStep i).applied.map (fun a => a.promotion_id) = (toApplyOf i).map (fun q => q.promotion_id) := by
  unfold finalStep
  rw [foldl_applied_ids]
  simp [initStep]

end Discount
