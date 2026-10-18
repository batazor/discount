import Proofs.Fold.Step
import Proofs.Lists
import Proofs.Allocate

/-!
# The fold invariant (invariants 1, 2, 5)

`FoldInv` is what every step of `applyOne` preserves; `foldl_inv` carries it through the fold.
-/
namespace Discount

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

end Discount
