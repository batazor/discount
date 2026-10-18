import Proofs.Compute

/-!
# Invariant 2: a discount never exceeds its cap, budget, the order cap or the subtotal
-/
namespace Discount

/-! ## 2. Discount never exceeds its cap -/

theorem discount_le_cap {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ (normalize i).promotions, p.promotion_id = a.promotion_id →
      ∀ c, p.cap = some c → a.discount.amount ≤ c.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid c hc
  rw [compute_applied] at ha
  obtain ⟨q, hq, hqid, hqcap, _⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub (normalize i) q hq) (by rw [hid, hqid])
  subst hpq
  exact hqcap c hc

theorem discount_le_budget {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ (normalize i).promotions, p.promotion_id = a.promotion_id →
      ∀ b, p.remaining_budget = some b → a.discount.amount ≤ b.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid b hb
  rw [compute_applied] at ha
  obtain ⟨q, hq, hqid, _, hqbud⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub (normalize i) q hq) (by rw [hid, hqid])
  subst hpq
  exact hqbud b hb

theorem total_discount_le_subtotal {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    r.total_discount.amount ≤ r.original_subtotal.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  rw [compute_total_discount, compute_original]
  have := lineRunning_sum_nonneg hv
  omega

theorem total_discount_eq_sum {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    r.total_discount.amount = (r.applied.map (fun a => a.discount.amount)).sum := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  rw [compute_total_discount, compute_applied]
  have := hinv.total
  simp only [sumDisc] at this
  omega

theorem orderCapInitial_amount {policy : PricingPolicy} {S : Int} {m : Money}
    (hm : policy.order_cap.amount = some m) :
    ∃ o0, orderCapInitial policy S = some o0 ∧ o0 ≤ m.amount := by
  unfold orderCapInitial
  rw [hm]
  cases policy.order_cap.bps with
  | none => exact ⟨m.amount, rfl, Int.le_refl _⟩
  | some b => exact ⟨_, rfl, Int.min_le_left _ _⟩

theorem orderCapInitial_bps {policy : PricingPolicy} {S : Int} {b : BasisPoints}
    (hb : policy.order_cap.bps = some b) :
    ∃ o0, orderCapInitial policy S = some o0 ∧ o0 ≤ roundDiv policy.rounding (S * b) BPS_SCALE := by
  unfold orderCapInitial
  rw [hb]
  cases policy.order_cap.amount with
  | none => exact ⟨_, rfl, Int.le_refl _⟩
  | some m => exact ⟨_, rfl, Int.min_le_right _ _⟩

theorem total_discount_le_order_cap {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    (∀ m, (normalize i).policy.order_cap.amount = some m → r.total_discount.amount ≤ m.amount) ∧
    (∀ b, i.policy.order_cap.bps = some b →
      r.total_discount.amount ≤ roundDiv i.policy.rounding (r.original_subtotal.amount * b) BPS_SCALE) := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  have hD : (compute (normalize i)).total_discount.amount = sumDisc (finalStep (normalize i)) := by
    rw [compute_total_discount, hinv.total]
  rw [hD, compute_original]
  constructor
  · intro m hm
    obtain ⟨o0, ho0, hle⟩ := orderCapInitial_amount (S := (subsOf (normalize i)).sum) hm
    have hrem := hinv.cap_some o0 ho0
    have := hinv.cap_nonneg _ hrem
    omega
  · intro b hb
    have hb' : (normalize i).policy.order_cap.bps = some b := hb
    obtain ⟨o0, ho0, hle⟩ := orderCapInitial_bps (S := (subsOf (normalize i)).sum) hb'
    have hr : roundDiv (normalize i).policy.rounding ((subsOf (normalize i)).sum * b) BPS_SCALE =
        roundDiv i.policy.rounding ((subsOf (normalize i)).sum * b) BPS_SCALE := rfl
    rw [hr] at hle
    have hrem := hinv.cap_some o0 ho0
    have := hinv.cap_nonneg _ hrem
    omega

end Discount
