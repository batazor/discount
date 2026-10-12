import Discount.Apply
import Discount.Theorems.Lists
import Discount.Theorems.Allocate
import Discount.Theorems.Rounding
import Discount.Theorems.Validate
import Discount.Theorems.Stacking

/-!
# The fold invariant (invariants 1, 2, 5)

`FoldInv` is what every step of `applyOne` preserves. The final theorems in
`Discount.Theorems.Invariants` read it off `finalStep`.
-/
namespace Discount

/-! ## clampDiscount and rawDiscount -/

theorem clampDiscount_spec (raw : Int) (cap budget ocap : Option Int) (running : Int)
    (hraw : 0 ≤ raw) (hcap : ∀ c, cap = some c → 0 ≤ c) (hbud : ∀ b, budget = some b → 0 ≤ b)
    (hoc : ∀ o, ocap = some o → 0 ≤ o) (hrun : 0 ≤ running) :
    let d := (clampDiscount raw cap budget ocap running).1
    0 ≤ d ∧ d ≤ running ∧ (∀ c, cap = some c → d ≤ c) ∧ (∀ b, budget = some b → d ≤ b) ∧
      (∀ o, ocap = some o → d ≤ o) := by
  unfold clampDiscount
  cases cap <;> cases budget <;> cases ocap <;> simp_all <;> omega

theorem rawDiscount_nonneg (policy : PricingPolicy) (original running : Int) (p : Promotion)
    (cur : Currency) (gids : List GroupId) (hp : PromotionOk cur gids p)
    (ho : 0 ≤ original) (hr : 0 ≤ running) : 0 ≤ rawDiscount policy original running p := by
  unfold rawDiscount
  have hpct : ∀ b : Int, 0 ≤ b → ∀ bps : Int, 0 ≤ bps →
      0 ≤ roundDiv (effectiveRounding policy p) (b * bps) BPS_SCALE :=
    fun b hb bps hbps => roundDiv_nonneg (effectiveRounding policy p) (b * bps) BPS_SCALE
      (Int.mul_nonneg hb hbps) (by decide)
  cases hact : p.action with
  | OrderPercentage bps =>
    have hb := (hp.percentage bps hact).1
    cases policy.stacking <;> simp only
    · exact hpct _ hr bps hb
    · exact hpct _ ho bps hb
  | OrderFixedAmount m =>
    cases policy.stacking <;> simp only <;> exact (hp.fixed m hact).2.1

/-! ## zipWith helpers for the allocation records -/

theorem zipAlloc_amounts (cur : Currency) (lines : List CartLine) (alloc : List Int)
    (h : lines.length = alloc.length) :
    (List.zipWith (fun (l : CartLine) (a : Int) =>
      ({ line_id := l.line_id, amount := ⟨a, cur⟩ } : LineAllocation)) lines alloc).map
        (fun x => x.amount.amount) = alloc := by
  induction lines generalizing alloc with
  | nil => cases alloc <;> simp_all
  | cons l ls ih =>
    cases alloc with
    | nil => simp at h
    | cons a as => simp [ih as (by simpa using h)]

theorem zipAlloc_ids (cur : Currency) (lines : List CartLine) (alloc : List Int)
    (h : lines.length = alloc.length) :
    (List.zipWith (fun (l : CartLine) (a : Int) =>
      ({ line_id := l.line_id, amount := ⟨a, cur⟩ } : LineAllocation)) lines alloc).map
        (fun x => x.line_id) = lines.map (fun l => l.line_id) := by
  induction lines generalizing alloc with
  | nil => cases alloc <;> simp_all
  | cons l ls ih =>
    cases alloc with
    | nil => simp at h
    | cons a as => simp [ih as (by simpa using h)]

/-! ## The invariant -/

/-- Facts about one applied record relative to the promotions `ps` it may come from. -/
structure AppliedOk (i : PricingInput) (subs : List Int) (ps : List Promotion)
    (a : AppliedPromotion) : Prop where
  src : ∃ p ∈ ps, p.promotion_id = a.promotion_id ∧
    (∀ c, p.cap = some c → a.discount.amount ≤ c.amount) ∧
    (∀ b, p.remaining_budget = some b → a.discount.amount ≤ b.amount)
  nonneg : 0 ≤ a.discount.amount
  alloc_sum : (a.allocation.map (fun x => x.amount.amount)).sum = a.discount.amount
  alloc_len : a.allocation.length = subs.length
  alloc_ids : a.allocation.map (fun x => x.line_id) = i.cart.lines.map (fun l => l.line_id)

def sumDisc (st : Step) : Int := (st.applied.map (fun a => a.discount.amount)).sum

structure FoldInv (i : PricingInput) (subs : List Int) (S : Int) (ps : List Promotion)
    (st : Step) : Prop where
  len : st.lineRunning.length = subs.length
  bounds : ∀ k, k < subs.length →
    0 ≤ st.lineRunning.getD k 0 ∧ st.lineRunning.getD k 0 ≤ subs.getD k 0
  total : sumDisc st = S - st.lineRunning.sum
  cap_some : ∀ o0, orderCapInitial i.policy S = some o0 →
    st.orderCapRemaining = some (o0 - sumDisc st)
  cap_none : orderCapInitial i.policy S = none → st.orderCapRemaining = none
  cap_nonneg : ∀ o, st.orderCapRemaining = some o → 0 ≤ o
  applied_ok : ∀ a ∈ st.applied, AppliedOk i subs ps a
  line_disc : ∀ k, k < subs.length →
    (st.applied.map (fun a => (a.allocation.map (fun x => x.amount.amount)).getD k 0)).sum =
      subs.getD k 0 - st.lineRunning.getD k 0

theorem lineRunning_nonneg {i subs S ps st} (h : FoldInv i subs S ps st) :
    ∀ x ∈ st.lineRunning, 0 ≤ x :=
  forall_mem_of_forall_getD _ _ (fun k hk => (h.bounds k (h.len ▸ hk)).1)

theorem sumDisc_append (st : Step) (a : AppliedPromotion) :
    (List.map (fun a => a.discount.amount) (st.applied ++ [a])).sum = sumDisc st + a.discount.amount := by
  simp [sumDisc, List.sum_append]

theorem applyOne_inv {i : PricingInput} {subs : List Int} {S : Int} {ps : List Promotion} {st : Step}
    (hinv : FoldInv i subs S ps st) (p : Promotion) (hp : p ∈ ps)
    (hok : PromotionOk i.cart.currency (i.groups.map (·.group_id)) p)
    (hsubs : ∀ x ∈ subs, 0 ≤ x) (hS : S = subs.sum) (hlen : subs.length = i.cart.lines.length) :
    FoldInv i subs S ps (applyOne i S st p) := by
  -- names for the pieces of the step
  have hlr0 : ∀ x ∈ st.lineRunning, 0 ≤ x := lineRunning_nonneg hinv
  have hrun : 0 ≤ st.lineRunning.sum := sum_nonneg_of_forall _ hlr0
  have hS0 : 0 ≤ S := hS ▸ sum_nonneg_of_forall _ hsubs
  have hraw := rawDiscount_nonneg i.policy S st.lineRunning.sum p _ _ hok hS0 hrun
  have hclamp := clampDiscount_spec (rawDiscount i.policy S st.lineRunning.sum p)
    (p.cap.map (·.amount)) (p.remaining_budget.map (·.amount)) st.orderCapRemaining
    st.lineRunning.sum hraw
    (fun c hc => by
      cases hcap : p.cap with
      | none => simp [hcap] at hc
      | some m => simp [hcap] at hc; subst hc; exact (hok.cap m hcap).2.1)
    (fun b hb => by
      cases hbud : p.remaining_budget with
      | none => simp [hbud] at hb
      | some m => simp [hbud] at hb; subst hb; exact (hok.budget m hbud).2.1)
    hinv.cap_nonneg hrun
  obtain ⟨hd0, hdrun, hdcap, hdbud, hdoc⟩ := hclamp
  -- the discount of this step
  generalize hdef : (clampDiscount (rawDiscount i.policy S st.lineRunning.sum p)
    (p.cap.map (·.amount)) (p.remaining_budget.map (·.amount)) st.orderCapRemaining
    st.lineRunning.sum) = dr at *
  have halloc_len := allocate_length dr.1 st.lineRunning
  have halloc_sum := allocate_sum dr.1 st.lineRunning hlr0 hd0 hdrun
  have halloc_le := allocate_le dr.1 st.lineRunning hlr0 hdrun
  have halloc_nn := allocate_nonneg dr.1 st.lineRunning hlr0 hd0
  have hlen' : st.lineRunning.length = (allocate dr.1 st.lineRunning).length := halloc_len.symm
  have hlines_len : i.cart.lines.length = (allocate dr.1 st.lineRunning).length := by
    rw [halloc_len, hinv.len, hlen]
  -- unfold the step
  simp only [applyOne, hdef]
  refine
    { len := ?_, bounds := ?_, total := ?_, cap_some := ?_, cap_none := ?_, cap_nonneg := ?_,
      applied_ok := ?_, line_disc := ?_ }
  · rw [subLists_length _ _ hlen', hinv.len]
  · intro k hk
    have hk' : k < st.lineRunning.length := hinv.len ▸ hk
    rw [subLists_getD _ _ hlen' k hk']
    have h1 := hinv.bounds k hk
    have h2 := halloc_le k hk'
    have h3 := halloc_nn k hk'
    omega
  · simp only [sumDisc, List.map_append, List.sum_append, List.map_cons, List.map_nil, List.sum_cons,
      List.sum_nil]
    rw [subLists_sum _ _ hlen', halloc_sum]
    have := hinv.total
    simp only [sumDisc] at this
    omega
  · intro o0 ho0
    have := hinv.cap_some o0 ho0
    rw [this]
    simp only [Option.map_some, sumDisc, List.map_append, List.sum_append, List.map_cons,
      List.map_nil, List.sum_cons, List.sum_nil, Option.some.injEq]
    simp only [sumDisc] at *
    omega
  · intro hnone
    rw [hinv.cap_none hnone]; rfl
  · intro o ho
    cases hoc : st.orderCapRemaining with
    | none => simp [hoc] at ho
    | some o' =>
      simp [hoc] at ho
      have := hdoc o' hoc
      omega
  · intro a ha
    rw [List.mem_append] at ha
    rcases ha with ha | ha
    · exact hinv.applied_ok a ha
    · simp at ha
      subst ha
      refine { src := ⟨p, hp, rfl, ?_, ?_⟩, nonneg := hd0, alloc_sum := ?_, alloc_len := ?_,
               alloc_ids := ?_ }
      · intro c hc; exact hdcap c.amount (by simp [hc])
      · intro b hb; exact hdbud b.amount (by simp [hb])
      · simp only
        rw [zipAlloc_amounts _ _ _ hlines_len, halloc_sum]
      · simp only [List.length_zipWith]
        rw [halloc_len, hinv.len, hlen, Nat.min_self]
      · simp only
        exact zipAlloc_ids _ _ _ hlines_len
  · intro k hk
    have hk' : k < st.lineRunning.length := hinv.len ▸ hk
    simp only [List.map_append, List.sum_append, List.map_cons, List.map_nil, List.sum_cons,
      List.sum_nil]
    rw [zipAlloc_amounts _ _ _ hlines_len, subLists_getD _ _ hlen' k hk', hinv.line_disc k hk]
    omega

theorem foldl_inv {i : PricingInput} {subs : List Int} {S : Int} {ps : List Promotion}
    (hsubs : ∀ x ∈ subs, 0 ≤ x) (hS : S = subs.sum) (hlen : subs.length = i.cart.lines.length)
    (hok : ∀ p ∈ ps, PromotionOk i.cart.currency (i.groups.map (·.group_id)) p) :
    ∀ (qs : List Promotion) (st : Step), (∀ q ∈ qs, q ∈ ps) → FoldInv i subs S ps st →
      FoldInv i subs S ps (qs.foldl (applyOne i S) st) := by
  intro qs
  induction qs with
  | nil => intro st _ h; simpa using h
  | cons q qs ih =>
    intro st hqs hinv
    rw [List.foldl_cons]
    exact ih _ (fun x hx => hqs x (by simp [hx]))
      (applyOne_inv hinv q (hqs q (by simp)) (hok q (hqs q (by simp))) hsubs hS hlen)

/-- The ids of the applied records are exactly the ids of the folded promotions, in order. -/
theorem foldl_applied_ids (i : PricingInput) (S : Int) :
    ∀ (qs : List Promotion) (st : Step),
      (qs.foldl (applyOne i S) st).applied.map (fun a => a.promotion_id) =
        st.applied.map (fun a => a.promotion_id) ++ qs.map (fun q => q.promotion_id) := by
  intro qs
  induction qs with
  | nil => intro st; simp
  | cons q qs ih =>
    intro st
    rw [List.foldl_cons, ih]
    simp [applyOne]

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
