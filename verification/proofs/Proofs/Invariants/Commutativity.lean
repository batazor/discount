import Proofs.Fold

/-!
# Invariant 3, commutativity part: where swapping two promotions does not change the order total

Stated on one pair of fold steps (`applyOne`), which is exactly what swapping the priorities
of two adjacent promotions in one stackable group changes. A step is *unclamped* when the
promotion has no cap and no budget, the state has no order cap, and the raw discount fits
the running total. Then:

- two steps whose raw discounts do not depend on the running total (fixed amounts under
  either policy, percentages under `Additive`) give the same `lineRunning.sum` in either
  order (`commute_of_running_independent` and its corollaries);
- two percentage steps under `Sequential` give totals that differ by at most 2 minor units,
  one per rounded step, for any rounding policies
  (`sequential_percentages_commute_upto_rounding`).

Nothing is claimed about the per-line allocation, about clamped steps, or about promotions
in different groups: there the order matters by design (ADR 4).
-/
namespace Discount

/-! ## One unclamped step -/

theorem clampDiscount_unclamped (raw running : Int) (h : raw ≤ running) :
    (clampDiscount raw none none none running).1 = raw := by
  simp [clampDiscount, Int.min_eq_left h]

/-- Without cap, budget and order cap, a raw discount that fits the running total is
subtracted exactly; lines stay non-negative and the order cap stays `none`. -/
theorem applyOne_unclamped (i : PricingInput) (S : Int) (st : Step) (p : Promotion)
    (hcap : p.cap = none) (hbud : p.remaining_budget = none) (hoc : st.orderCapRemaining = none)
    (hlr : ∀ x ∈ st.lineRunning, 0 ≤ x)
    (hraw0 : 0 ≤ rawDiscount i.policy S st.lineRunning.sum p)
    (hfit : rawDiscount i.policy S st.lineRunning.sum p ≤ st.lineRunning.sum) :
    (applyOne i S st p).lineRunning.sum =
        st.lineRunning.sum - rawDiscount i.policy S st.lineRunning.sum p ∧
      (∀ x ∈ (applyOne i S st p).lineRunning, 0 ≤ x) ∧
      (applyOne i S st p).orderCapRemaining = none := by
  have hd : (clampDiscount (rawDiscount i.policy S st.lineRunning.sum p) (p.cap.map (·.amount))
      (p.remaining_budget.map (·.amount)) st.orderCapRemaining st.lineRunning.sum).1 =
      rawDiscount i.policy S st.lineRunning.sum p := by
    rw [hcap, hbud, hoc]
    exact clampDiscount_unclamped _ _ hfit
  simp only [applyOne, hd]
  generalize hr : rawDiscount i.policy S st.lineRunning.sum p = d at *
  have hlen' : st.lineRunning.length = (allocate d st.lineRunning).length :=
    (allocate_length d st.lineRunning).symm
  refine ⟨?_, ?_, ?_⟩
  · rw [subLists_sum _ _ hlen', allocate_sum d st.lineRunning hlr hraw0 hfit]
  · apply forall_mem_of_forall_getD
    intro k hk
    rw [subLists_length _ _ hlen'] at hk
    rw [subLists_getD _ _ hlen' k hk]
    have := allocate_le d st.lineRunning hlr hfit k hk
    omega
  · simp [hoc]

/-! ## Raw discounts of the three shapes -/

theorem rawDiscount_fixed (policy : PricingPolicy) (S r : Int) (p : Promotion) (m : Money)
    (h : p.action = .OrderFixedAmount m) : rawDiscount policy S r p = m.amount := by
  unfold rawDiscount
  cases policy.stacking <;> simp [h]

theorem rawDiscount_additive (policy : PricingPolicy) (S r : Int) (p : Promotion) (bps : Int)
    (hs : policy.stacking = .Additive) (h : p.action = .OrderPercentage bps) :
    rawDiscount policy S r p = roundDiv (effectiveRounding policy p) (S * bps) BPS_SCALE := by
  unfold rawDiscount
  rw [hs]
  simp [h]

theorem rawDiscount_sequential (policy : PricingPolicy) (S r : Int) (p : Promotion) (bps : Int)
    (hs : policy.stacking = .Sequential) (h : p.action = .OrderPercentage bps) :
    rawDiscount policy S r p = roundDiv (effectiveRounding policy p) (r * bps) BPS_SCALE := by
  unfold rawDiscount
  rw [hs]
  simp [h]

/-! ## Running-independent discounts commute exactly -/

theorem commute_of_running_independent (i : PricingInput) (S : Int) (st : Step) (a b : Promotion)
    (da db : Int) (hA : ∀ r, rawDiscount i.policy S r a = da)
    (hB : ∀ r, rawDiscount i.policy S r b = db)
    (hca : a.cap = none) (hba : a.remaining_budget = none)
    (hcb : b.cap = none) (hbb : b.remaining_budget = none)
    (hoc : st.orderCapRemaining = none) (hlr : ∀ x ∈ st.lineRunning, 0 ≤ x)
    (hda : 0 ≤ da) (hdb : 0 ≤ db) (hfit : da + db ≤ st.lineRunning.sum) :
    (applyOne i S (applyOne i S st a) b).lineRunning.sum =
      (applyOne i S (applyOne i S st b) a).lineRunning.sum := by
  obtain ⟨s1, n1, o1⟩ := applyOne_unclamped i S st a hca hba hoc hlr
    (by rw [hA]; exact hda) (by rw [hA]; omega)
  obtain ⟨s2, -, -⟩ := applyOne_unclamped i S (applyOne i S st a) b hcb hbb o1 n1
    (by rw [hB]; exact hdb) (by rw [hB, s1, hA]; omega)
  obtain ⟨t1, m1, q1⟩ := applyOne_unclamped i S st b hcb hbb hoc hlr
    (by rw [hB]; exact hdb) (by rw [hB]; omega)
  obtain ⟨t2, -, -⟩ := applyOne_unclamped i S (applyOne i S st b) a hca hba q1 m1
    (by rw [hA]; exact hda) (by rw [hA, t1, hB]; omega)
  rw [s2, s1, hA, hB, t2, t1, hA, hB]
  omega

/-- Two fixed amounts, under either stacking policy. -/
theorem fixed_amounts_commute (i : PricingInput) (S : Int) (st : Step) (a b : Promotion)
    (ma mb : Money) (ha : a.action = .OrderFixedAmount ma) (hb : b.action = .OrderFixedAmount mb)
    (hca : a.cap = none) (hba : a.remaining_budget = none)
    (hcb : b.cap = none) (hbb : b.remaining_budget = none)
    (hoc : st.orderCapRemaining = none) (hlr : ∀ x ∈ st.lineRunning, 0 ≤ x)
    (hma : 0 ≤ ma.amount) (hmb : 0 ≤ mb.amount) (hfit : ma.amount + mb.amount ≤ st.lineRunning.sum) :
    (applyOne i S (applyOne i S st a) b).lineRunning.sum =
      (applyOne i S (applyOne i S st b) a).lineRunning.sum :=
  commute_of_running_independent i S st a b ma.amount mb.amount
    (fun r => rawDiscount_fixed i.policy S r a ma ha) (fun r => rawDiscount_fixed i.policy S r b mb hb)
    hca hba hcb hbb hoc hlr hma hmb hfit

/-- Two percentages under `Additive`: both read the original subtotal `S`. -/
theorem additive_percentages_commute (i : PricingInput) (S : Int) (st : Step) (a b : Promotion)
    (pa pb : Int) (hs : i.policy.stacking = .Additive)
    (ha : a.action = .OrderPercentage pa) (hb : b.action = .OrderPercentage pb)
    (hpa : 0 ≤ pa) (hpb : 0 ≤ pb) (hS : 0 ≤ S)
    (hca : a.cap = none) (hba : a.remaining_budget = none)
    (hcb : b.cap = none) (hbb : b.remaining_budget = none)
    (hoc : st.orderCapRemaining = none) (hlr : ∀ x ∈ st.lineRunning, 0 ≤ x)
    (hfit : roundDiv (effectiveRounding i.policy a) (S * pa) BPS_SCALE +
        roundDiv (effectiveRounding i.policy b) (S * pb) BPS_SCALE ≤ st.lineRunning.sum) :
    (applyOne i S (applyOne i S st a) b).lineRunning.sum =
      (applyOne i S (applyOne i S st b) a).lineRunning.sum :=
  commute_of_running_independent i S st a b _ _
    (fun r => rawDiscount_additive i.policy S r a pa hs ha)
    (fun r => rawDiscount_additive i.policy S r b pb hs hb)
    hca hba hcb hbb hoc hlr
    (roundDiv_nonneg _ _ _ (Int.mul_nonneg hS hpa) (by decide))
    (roundDiv_nonneg _ _ _ (Int.mul_nonneg hS hpb) (by decide)) hfit

/-! ## Sequential percentages commute up to rounding -/

/-- The integer core: with `ε = den·rd(x) − x ∈ (−den, den/2]` for each of the four
roundings (`den = 10000`), the two totals differ by less than 3 units. -/
theorem seq_two_step_bound (R pa pb da db' db da' : Int)
    (hpa0 : 0 ≤ pa) (hpa1 : pa ≤ 10000) (hpb0 : 0 ≤ pb) (hpb1 : pb ≤ 10000)
    (h1 : -10000 < 10000 * da - R * pa) (h2 : 2 * (10000 * da - R * pa) ≤ 10000)
    (h3 : -10000 < 10000 * db' - (R - da) * pb) (h4 : 2 * (10000 * db' - (R - da) * pb) ≤ 10000)
    (h5 : -10000 < 10000 * db - R * pb) (h6 : 2 * (10000 * db - R * pb) ≤ 10000)
    (h7 : -10000 < 10000 * da' - (R - db) * pa) (h8 : 2 * (10000 * da' - (R - db) * pa) ≤ 10000) :
    (da + db') - (db + da') ≤ 2 ∧ (db + da') - (da + db') ≤ 2 := by
  have eA : (R - da) * pb = R * pb - da * pb := Int.sub_mul R da pb
  have eB : (R - db) * pa = R * pa - db * pa := Int.sub_mul R db pa
  have eC : 10000 * (da * pb) = (10000 * da - R * pa) * pb + (R * pa) * pb := by
    rw [Int.sub_mul, Int.mul_assoc]; omega
  have eD : 10000 * (db * pa) = (10000 * db - R * pb) * pa + (R * pb) * pa := by
    rw [Int.sub_mul, Int.mul_assoc]; omega
  have eE : (R * pb) * pa = (R * pa) * pb := by
    rw [Int.mul_assoc, Int.mul_comm pb pa, Int.mul_assoc]
  have b1 : (-10000) * (10000 - pb) ≤ (10000 * da - R * pa) * (10000 - pb) :=
    Int.mul_le_mul_of_nonneg_right (by omega) (by omega)
  have b2 : (2 * (10000 * da - R * pa)) * (10000 - pb) ≤ 10000 * (10000 - pb) :=
    Int.mul_le_mul_of_nonneg_right h2 (by omega)
  have b3 : (-10000) * (10000 - pa) ≤ (10000 * db - R * pb) * (10000 - pa) :=
    Int.mul_le_mul_of_nonneg_right (by omega) (by omega)
  have b4 : (2 * (10000 * db - R * pb)) * (10000 - pa) ≤ 10000 * (10000 - pa) :=
    Int.mul_le_mul_of_nonneg_right h6 (by omega)
  have x1 : (10000 * da - R * pa) * (10000 - pb) =
      (10000 * da - R * pa) * 10000 - (10000 * da - R * pa) * pb := Int.mul_sub _ _ _
  have x2 : (2 * (10000 * da - R * pa)) * (10000 - pb) =
      2 * ((10000 * da - R * pa) * (10000 - pb)) := Int.mul_assoc _ _ _
  have x3 : (10000 * db - R * pb) * (10000 - pa) =
      (10000 * db - R * pb) * 10000 - (10000 * db - R * pb) * pa := Int.mul_sub _ _ _
  have x4 : (2 * (10000 * db - R * pb)) * (10000 - pa) =
      2 * ((10000 * db - R * pb) * (10000 - pa)) := Int.mul_assoc _ _ _
  have y1 : (10000 * da - R * pa) * 10000 = 10000 * (10000 * da - R * pa) := Int.mul_comm _ _
  have y2 : (10000 * db - R * pb) * 10000 = 10000 * (10000 * db - R * pb) := Int.mul_comm _ _
  constructor <;> omega

theorem sequential_percentages_commute_upto_rounding (i : PricingInput) (S : Int) (st : Step)
    (a b : Promotion) (pa pb : Int) (hs : i.policy.stacking = .Sequential)
    (ha : a.action = .OrderPercentage pa) (hb : b.action = .OrderPercentage pb)
    (hpa0 : 0 ≤ pa) (hpa1 : pa ≤ BPS_SCALE) (hpb0 : 0 ≤ pb) (hpb1 : pb ≤ BPS_SCALE)
    (hca : a.cap = none) (hba : a.remaining_budget = none)
    (hcb : b.cap = none) (hbb : b.remaining_budget = none)
    (hoc : st.orderCapRemaining = none) (hlr : ∀ x ∈ st.lineRunning, 0 ≤ x) :
    (applyOne i S (applyOne i S st a) b).lineRunning.sum -
        (applyOne i S (applyOne i S st b) a).lineRunning.sum ≤ 2 ∧
      (applyOne i S (applyOne i S st b) a).lineRunning.sum -
        (applyOne i S (applyOne i S st a) b).lineRunning.sum ≤ 2 := by
  have hR : 0 ≤ st.lineRunning.sum := sum_nonneg_of_forall _ hlr
  have hden : (0 : Int) < BPS_SCALE := by decide
  -- a then b
  have ra := rawDiscount_sequential i.policy S st.lineRunning.sum a pa hs ha
  obtain ⟨s1, n1, o1⟩ := applyOne_unclamped i S st a hca hba hoc hlr
    (by rw [ra]; exact roundDiv_nonneg _ _ _ (Int.mul_nonneg hR hpa0) hden)
    (by rw [ra]; exact roundDiv_le _ _ _ hR hpa0 hpa1)
  have R1 : 0 ≤ (applyOne i S st a).lineRunning.sum := sum_nonneg_of_forall _ n1
  have rb' := rawDiscount_sequential i.policy S (applyOne i S st a).lineRunning.sum b pb hs hb
  obtain ⟨s2, -, -⟩ := applyOne_unclamped i S (applyOne i S st a) b hcb hbb o1 n1
    (by rw [rb']; exact roundDiv_nonneg _ _ _ (Int.mul_nonneg R1 hpb0) hden)
    (by rw [rb']; exact roundDiv_le _ _ _ R1 hpb0 hpb1)
  -- b then a
  have rb := rawDiscount_sequential i.policy S st.lineRunning.sum b pb hs hb
  obtain ⟨t1, m1, q1⟩ := applyOne_unclamped i S st b hcb hbb hoc hlr
    (by rw [rb]; exact roundDiv_nonneg _ _ _ (Int.mul_nonneg hR hpb0) hden)
    (by rw [rb]; exact roundDiv_le _ _ _ hR hpb0 hpb1)
  have R2 : 0 ≤ (applyOne i S st b).lineRunning.sum := sum_nonneg_of_forall _ m1
  have ra' := rawDiscount_sequential i.policy S (applyOne i S st b).lineRunning.sum a pa hs ha
  obtain ⟨t2, -, -⟩ := applyOne_unclamped i S (applyOne i S st b) a hca hba q1 m1
    (by rw [ra']; exact roundDiv_nonneg _ _ _ (Int.mul_nonneg R2 hpa0) hden)
    (by rw [ra']; exact roundDiv_le _ _ _ R2 hpa0 hpa1)
  -- the four rounding errors
  have e1 := roundDiv_err (effectiveRounding i.policy a) (st.lineRunning.sum * pa)
  have e2 := roundDiv_err (effectiveRounding i.policy b) ((applyOne i S st a).lineRunning.sum * pb)
  have e3 := roundDiv_err (effectiveRounding i.policy b) (st.lineRunning.sum * pb)
  have e4 := roundDiv_err (effectiveRounding i.policy a) ((applyOne i S st b).lineRunning.sum * pa)
  rw [s2, rb', s1, ra, t2, ra', t1, rb]
  rw [s1, ra] at e2
  rw [t1, rb] at e4
  have hB : BPS_SCALE = 10000 := rfl
  rw [hB] at e1 e2 e3 e4 hpa1 hpb1 ⊢
  have := seq_two_step_bound st.lineRunning.sum pa pb _ _ _ _ hpa0 hpa1 hpb0 hpb1
    e1.1 e1.2 e2.1 e2.2 e3.1 e3.2 e4.1 e4.2
  omega

end Discount
