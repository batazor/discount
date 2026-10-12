import Discount.Theorems.Fold

/-!
# The invariants of `docs/invariants.md`, stated on `apply input = .ok result`

Numbering follows the document. Everything here is derived from `finalStep_inv` (the fold
invariant) and the stage lemmas; nothing is assumed about the input beyond `validate = ok`.
-/
namespace Discount

/-! ## Plumbing -/

theorem apply_ok {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    validate i = .ok () ∧ r = compute i := by
  unfold apply at h
  split at h
  · exact nomatch h
  · exact ⟨by assumption, (Except.ok.inj h).symm⟩

theorem getD_eq_getElem (l : List Int) (k : Nat) (hk : k < l.length) : l.getD k 0 = l[k] := by
  rw [List.getD_eq_getElem?_getD, List.getElem?_eq_getElem hk]; rfl

theorem eq_of_nodup_map {α β : Type} {l : List α} {f : α → β} (h : (l.map f).Nodup)
    {a b : α} (ha : a ∈ l) (hb : b ∈ l) (hf : f a = f b) : a = b := by
  induction l with
  | nil => simp at ha
  | cons x xs ih =>
    rw [List.map_cons, List.nodup_cons] at h
    rcases List.mem_cons.mp ha with rfl | ha' <;> rcases List.mem_cons.mp hb with rfl | hb'
    · rfl
    · exact absurd (hf ▸ List.mem_map_of_mem hb') h.1
    · exact absurd (hf ▸ List.mem_map_of_mem ha') h.1
    · exact ih h.2 ha' hb'

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
  have hk' : k < (subsOf i).length := by
    simp only [List.length_zipWith, List.length_zip] at hk; omega
  have hb := hinv.bounds k hk'
  rw [getD_eq_getElem _ _ (hinv.len ▸ hk'), getD_eq_getElem _ _ hk'] at hb
  simp only [List.getElem_zipWith, List.getElem_zip]
  omega

/-! ## 2. Discount never exceeds its cap -/

theorem discount_le_cap {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ i.promotions, p.promotion_id = a.promotion_id →
      ∀ c, p.cap = some c → a.discount.amount ≤ c.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid c hc
  rw [compute_applied] at ha
  obtain ⟨q, hq, hqid, hqcap, _⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub i q hq) (by rw [hid, hqid])
  subst hpq
  exact hqcap c hc

theorem discount_le_budget {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ i.promotions, p.promotion_id = a.promotion_id →
      ∀ b, p.remaining_budget = some b → a.discount.amount ≤ b.amount := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid b hb
  rw [compute_applied] at ha
  obtain ⟨q, hq, hqid, _, hqbud⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub i q hq) (by rw [hid, hqid])
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
    (∀ m, i.policy.order_cap.amount = some m → r.total_discount.amount ≤ m.amount) ∧
    (∀ b, i.policy.order_cap.bps = some b →
      r.total_discount.amount ≤ roundDiv i.policy.rounding (r.original_subtotal.amount * b) BPS_SCALE) := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  have hD : (compute i).total_discount.amount = sumDisc (finalStep i) := by
    rw [compute_total_discount, hinv.total]
  rw [hD, compute_original]
  constructor
  · intro m hm
    obtain ⟨o0, ho0, hle⟩ := orderCapInitial_amount (S := (subsOf i).sum) hm
    have hrem := hinv.cap_some o0 ho0
    have := hinv.cap_nonneg _ hrem
    omega
  · intro b hb
    obtain ⟨o0, ho0, hle⟩ := orderCapInitial_bps (S := (subsOf i).sum) hb
    have hrem := hinv.cap_some o0 ho0
    have := hinv.cap_nonneg _ hrem
    omega

/-! ## 4. Exclusive promotions never combine; first-match groups apply at most one -/

theorem applied_length {i : PricingInput} : (finalStep i).applied.length = (toApplyOf i).length := by
  have := congrArg List.length (finalStep_ids i)
  simpa using this

theorem exclusive_alone {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied, ∀ p ∈ i.promotions, p.promotion_id = a.promotion_id → p.exclusive = true →
      r.applied.length = 1 := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hinv := finalStep_inv hv
  intro a ha p hp hid hex
  rw [compute_applied] at ha ⊢
  obtain ⟨q, hq, hqid, _, _⟩ := (hinv.applied_ok a ha).src
  have hpq : p = q := eq_of_nodup_map hv.promotion_ids hp (toApply_sub i q hq) (by rw [hid, hqid])
  have hexq : q.exclusive = true := hpq ▸ hex
  -- q is in the first-match output, hence in the exclusive output, which is then [q]
  have hq1 : q ∈ (afterExclusive i).1 := selectFirstMatch_mem hq
  have hex1 : (afterExclusive i).1 = [q] := selectExclusive_exclusive hq1 hexq
  have hle : (toApplyOf i).length ≤ 1 := by
    have := selectFirstMatch_length_le i.groups (afterExclusive i).1 []
    rw [hex1] at this
    simpa [toApplyOf, afterFirstMatch, hex1] using this
  have hpos : 0 < (finalStep i).applied.length := List.length_pos_of_mem ha
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
      (r.applied.filter (appliedInGroup i gid)).length ≤ 1 := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  intro gid hmode
  rw [compute_applied]
  have hunf : appliedInGroup i gid = fun a => groupOfPid i gid a.promotion_id := by
    funext a; rfl
  rw [hunf]
  rw [filter_length_eq_of_map_eq (finalStep i).applied (toApplyOf i)
    (fun a => a.promotion_id) (fun q => q.promotion_id) (groupOfPid i gid) (finalStep_ids i)]
  have h2 : (toApplyOf i).filter (fun q => groupOfPid i gid q.promotion_id) =
      (toApplyOf i).filter (fun q => q.group_id == gid) := by
    apply List.filter_congr
    intro q hq
    unfold groupOfPid
    rw [find?_eq_some_of_nodup hv.promotion_ids (toApply_sub i q hq)]
  rw [h2]
  exact selectFirstMatch_at_most_one_nil i.groups gid hmode _

/-! ## 5. Allocation is conservative -/

theorem allocation_conserves {i : PricingInput} {r : PricingResult} (h : apply i = .ok r) :
    ∀ a ∈ r.applied,
      (a.allocation.map (fun x => x.amount.amount)).sum = a.discount.amount ∧
      a.allocation.length = r.lines.length ∧
      a.allocation.map (fun x => x.line_id) = i.cart.lines.map (fun l => l.line_id) := by
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
  have hk' : k < (subsOf i).length := by rw [← compute_lines_length hv]; exact hk
  rw [compute_applied, hinv.line_disc k hk', compute_lines]
  exact zipLines_discount_getD _ _ _ _ k (subsOf_length i).symm hinv.len.symm
    (by rw [← subsOf_length]; exact hk')

/-! ## 7. Eligibility depends only on its inputs -/

theorem ruleHolds_congr (r : Rule) (i j : PricingInput) (h : i.now = j.now) :
    ruleHolds r i = ruleHolds r j := by
  cases r; simp [ruleHolds, h]

theorem firstFailingRule_congr (rs : List Rule) (i j : PricingInput) (h : i.now = j.now) (k : Nat) :
    firstFailingRule rs i k = firstFailingRule rs j k := by
  induction rs generalizing k with
  | nil => rfl
  | cons r rs ih => simp [firstFailingRule, ruleHolds_congr r i j h, ih]

/-- In the first iteration eligibility reads nothing but `now` (and, trivially, the cart and
customer context, which no rule inspects yet). -/
theorem eligible_depends_on_input (p : Promotion) (i j : PricingInput) (h : i.now = j.now) :
    eligible p i = eligible p j := by
  simp [eligible, firstFailingRule_congr p.rules i j h]

end Discount
