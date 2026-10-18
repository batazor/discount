import Proofs.Compute

/-!
# Invariant 4: exclusive promotions never combine; first-match groups apply at most one
-/
namespace Discount

/-! ## 4. Exclusive promotions never combine; first-match groups apply at most one -/

theorem applied_length {i : PricingInput} : (finalStep i).applied.length = (toApplyOf i).length := by
  have := congrArg List.length (finalStep_ids i)
  simpa using this

theorem exclusive_alone {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ (normalize i).promotions, p.promotion_id = a.promotion_id → p.exclusive = true →
      r.applied.length = 1 := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid hex
  rw [compute_applied] at ha ⊢
  obtain ⟨q, hq, hqid, _, _⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub (normalize i) q hq) (by rw [hid, hqid])
  have hexq : q.exclusive = true := hpq ▸ hex
  -- q is in the first-match output, hence in the exclusive output, which is then [q]
  have hq1 : q ∈ (afterExclusive (normalize i)).1 := selectFirstMatch_mem hq
  have hex1 : (afterExclusive (normalize i)).1 = [q] := selectExclusive_exclusive hq1 hexq
  have hle : (toApplyOf (normalize i)).length ≤ 1 := by
    have := selectFirstMatch_length_le (normalize i).groups (afterExclusive (normalize i)).1 []
    rw [hex1] at this
    simpa [toApplyOf, afterFirstMatch, hex1] using this
  have hpos : 0 < (finalStep (normalize i)).applied.length := List.length_pos_of_mem ha
  rw [applied_length] at hpos ⊢
  omega

/-- The group of a promotion id, looked up in the input. -/
def groupOfPid (i : PricingInput) (gid : GroupId) (pid : PromotionId) : Bool :=
  match i.promotions.find? (fun p => p.promotion_id == pid) with
  | some p => p.group_id == gid
  | none => false

/-- The group of an applied record, looked up through its source promotion. -/
def appliedInGroup (i : PricingInput) (gid : GroupId) (a : AppliedPromotion) : Bool :=
  groupOfPid i gid a.promotion_id

theorem filter_length_eq_of_map_eq {α β γ : Type} (as : List α) (qs : List β) (fa : α → γ) (fq : β → γ)
    (g : γ → Bool) (h : as.map fa = qs.map fq) :
    (as.filter (fun a => g (fa a))).length = (qs.filter (fun q => g (fq q))).length := by
  rw [← List.countP_eq_length_filter, ← List.countP_eq_length_filter]
  have h1 : (as.map fa).countP g = as.countP (fun a => g (fa a)) := List.countP_map
  have h2 : (qs.map fq).countP g = qs.countP (fun q => g (fq q)) := List.countP_map
  rw [← h1, ← h2, h]

theorem first_match_at_most_one {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ gid, modeOf i.groups gid = .FirstMatch →
      (r.applied.filter (appliedInGroup (normalize i) gid)).length ≤ 1 := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  intro gid hmode
  rw [compute_applied]
  have hunf : appliedInGroup (normalize i) gid = fun a => groupOfPid (normalize i) gid a.promotion_id := by
    funext a; rfl
  rw [hunf]
  rw [filter_length_eq_of_map_eq (finalStep (normalize i)).applied (toApplyOf (normalize i))
    (fun a => a.promotion_id) (fun q => q.promotion_id) (groupOfPid (normalize i) gid) (finalStep_ids (normalize i))]
  have h2 : (toApplyOf (normalize i)).filter (fun q => groupOfPid (normalize i) gid q.promotion_id) =
      (toApplyOf (normalize i)).filter (fun q => q.group_id == gid) := by
    apply List.filter_congr
    intro q hq
    unfold groupOfPid
    rw [find?_eq_some_of_nodup hv.promotion_ids (toApply_sub (normalize i) q hq)]
  rw [h2]
  exact selectFirstMatch_at_most_one_nil (normalize i).groups gid hmode _

end Discount
