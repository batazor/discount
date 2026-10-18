import Discount.Apply
import Proofs.Rounding
import Proofs.Validate

/-!
# One step: `clampDiscount`, `rawDiscount` and the allocation records
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

end Discount
