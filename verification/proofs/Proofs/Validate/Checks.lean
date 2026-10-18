import Discount.Apply.Normalize

/-!
# What the cart and money checks guarantee (ADR 6)

`MoneyOk` and `LineOk` are the facts the arithmetic proofs read; they are stated on the
normalized values.
-/
namespace Discount

theorem andThen_ok {a b : Except PricingError Unit} (h : andThen a b = .ok ()) :
    a = .ok () ∧ b = .ok () := by
  unfold andThen at h
  split at h
  · exact nomatch h
  · exact ⟨rfl, h⟩

/-- A money value in the cart currency, within bounds. -/
abbrev MoneyOk (cur : Currency) (m : Money) : Prop :=
  m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS

theorem checkMoney_ok {m : Money} {cur : Currency} {rates : List ExchangeRate} {f : String}
    (h : checkMoney m cur rates f = .ok ()) : MoneyOk cur (normalizeMoney rates cur m) := by
  unfold checkMoney at h
  split at h
  · exact nomatch h
  · split at h
    · exact nomatch h
    · rename_i c hc
      split at h
      · exact nomatch h
      · split at h
        · exact nomatch h
        · rename_i _ h2
          unfold normalizeMoney
          rw [hc]
          simp [amountInRange] at h2
          exact ⟨rfl, h2.1, h2.2⟩

theorem checkOptMoney_ok {m : Option Money} {cur : Currency} {rates : List ExchangeRate} {f : String}
    (h : checkOptMoney m cur rates f = .ok ()) :
    ∀ y, m.map (normalizeMoney rates cur) = some y → MoneyOk cur y := by
  intro y hy
  cases m with
  | none => simp at hy
  | some x =>
    simp at hy
    subst hy
    exact checkMoney_ok h

/-- Facts about one validated line. -/
structure LineOk (cur : Currency) (l : CartLine) : Prop where
  currency : l.unit_price.currency = cur
  amount_nonneg : 0 ≤ l.unit_price.amount
  amount_le : l.unit_price.amount ≤ MAX_MINOR_UNITS
  quantity_pos : 1 ≤ l.quantity
  quantity_le : l.quantity ≤ MAX_QUANTITY

theorem checkLines_ok {ls : List CartLine} {cur : Currency} {k : Nat}
    (h : checkLines ls cur k = .ok ()) : ∀ l ∈ ls, LineOk cur l := by
  induction ls generalizing k with
  | nil => intro l hl; simp at hl
  | cons l ls ih =>
    intro x hx
    simp only [checkLines] at h
    split at h
    · exact nomatch h
    · split at h
      · exact nomatch h
      · split at h
        · exact nomatch h
        · split at h
          · exact nomatch h
          · rename_i h1 h2 h3 h4
            rcases List.mem_cons.mp hx with rfl | hx'
            · simp [amountInRange] at h2 h3 h4
              exact ⟨h2, h3.1, h3.2, h4.1, h4.2⟩
            · exact ih h x hx'

theorem firstDuplicate_none {l seen : List String} (h : firstDuplicate l seen = none) :
    l.Nodup ∧ ∀ x ∈ l, x ∉ seen := by
  induction l generalizing seen with
  | nil => simp
  | cons x xs ih =>
    simp only [firstDuplicate] at h
    split at h
    · exact nomatch h
    · rename_i hx
      obtain ⟨hnd, hns⟩ := ih h
      refine ⟨List.nodup_cons.mpr ⟨fun hm => ?_, hnd⟩, ?_⟩
      · exact hns x hm (by simp)
      · intro y hy
        rcases List.mem_cons.mp hy with rfl | hy'
        · exact hx
        · intro hys; exact hns y hy' (by simp [hys])

theorem checkDuplicates_ok {ids : List String} {mk : String → PricingError}
    (h : checkDuplicates ids mk = .ok ()) : ids.Nodup := by
  unfold checkDuplicates at h
  split at h
  · exact nomatch h
  · exact (firstDuplicate_none (by assumption)).1

theorem checkOrderCap_ok {oc : OrderCap} {cur : Currency} {rates : List ExchangeRate}
    (h : checkOrderCap oc cur rates = .ok ()) :
    (∀ y, oc.amount.map (normalizeMoney rates cur) = some y → MoneyOk cur y) ∧
    (∀ b, oc.bps = some b → 0 ≤ b ∧ b ≤ BPS_SCALE) := by
  unfold checkOrderCap at h
  obtain ⟨h1, h2⟩ := andThen_ok h
  refine ⟨checkOptMoney_ok h1, ?_⟩
  intro b hb
  rw [hb] at h2
  dsimp only at h2
  split at h2
  · exact nomatch h2
  · rename_i hr
    simpa [bpsInRange] using hr

end Discount
