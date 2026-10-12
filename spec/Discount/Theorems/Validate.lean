import Discount.Validate

/-!
# What `validate = ok` guarantees (ADR 6)

`validate_ok` is the bridge from the sequential checks to the facts the arithmetic proofs
need. Only the facts actually used downstream are collected.
-/
namespace Discount

theorem andThen_ok {a b : Except PricingError Unit} (h : andThen a b = .ok ()) :
    a = .ok () ∧ b = .ok () := by
  unfold andThen at h
  split at h
  · exact nomatch h
  · exact ⟨rfl, h⟩

theorem checkMoney_ok {m : Money} {cur : Currency} {f : String} (h : checkMoney m cur f = .ok ()) :
    m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS := by
  unfold checkMoney at h
  split at h
  · exact nomatch h
  · split at h
    · exact nomatch h
    · split at h
      · exact nomatch h
      · rename_i h1 h2 h3
        simp [amountInRange] at h2 h3
        exact ⟨h2, h3⟩

theorem checkOptMoney_ok {m : Option Money} {cur : Currency} {f : String}
    (h : checkOptMoney m cur f = .ok ()) :
    ∀ x, m = some x → x.currency = cur ∧ 0 ≤ x.amount ∧ x.amount ≤ MAX_MINOR_UNITS := by
  intro x hx
  subst hx
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

theorem checkOrderCap_ok {oc : OrderCap} {cur : Currency} (h : checkOrderCap oc cur = .ok ()) :
    (∀ m, oc.amount = some m → m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS) ∧
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

/-- Facts about one validated promotion. -/
structure PromotionOk (cur : Currency) (gids : List GroupId) (p : Promotion) : Prop where
  percentage : ∀ b, p.action = .OrderPercentage b → 0 ≤ b ∧ b ≤ BPS_SCALE
  fixed : ∀ m, p.action = .OrderFixedAmount m → m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS
  cap : ∀ m, p.cap = some m → m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS
  budget : ∀ m, p.remaining_budget = some m → m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS
  group : p.group_id ∈ gids

theorem checkAction_ok {a : Action} {cur : Currency} {i : Nat} (h : checkAction a cur i = .ok ()) :
    (∀ b, a = .OrderPercentage b → 0 ≤ b ∧ b ≤ BPS_SCALE) ∧
    (∀ m, a = .OrderFixedAmount m → m.currency = cur ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS) := by
  unfold checkAction at h
  split at h
  · rename_i b
    split at h
    · exact nomatch h
    · rename_i hr
      refine ⟨fun b' hb => ?_, (fun m hm => nomatch hm)⟩
      have hbb := Action.OrderPercentage.inj hb
      subst hbb
      simpa [bpsInRange] using hr
  · rename_i m
    refine ⟨(fun b hb => nomatch hb), fun m' hm => ?_⟩
    have hmm := Action.OrderFixedAmount.inj hm
    subst hmm
    exact checkMoney_ok h

theorem checkPromotions_ok {ps : List Promotion} {cur : Currency} {gids : List GroupId} {k : Nat}
    (h : checkPromotions ps cur gids k = .ok ()) : ∀ p ∈ ps, PromotionOk cur gids p := by
  induction ps generalizing k with
  | nil => intro p hp; simp at hp
  | cons p ps ih =>
    intro x hx
    simp only [checkPromotions] at h
    obtain ⟨h1, h2⟩ := andThen_ok h
    obtain ⟨h3, h4⟩ := andThen_ok h2
    obtain ⟨h5, h6⟩ := andThen_ok h4
    obtain ⟨h7, h8⟩ := andThen_ok h6
    rcases List.mem_cons.mp hx with rfl | hx'
    · obtain ⟨ha1, ha2⟩ := checkAction_ok h1
      refine { percentage := ha1, fixed := ha2, cap := checkOptMoney_ok h3,
               budget := checkOptMoney_ok h5, group := ?_ }
      split at h7
      · assumption
      · exact nomatch h7
    · exact ih h8 x hx'

/-- Everything the proofs need from a validated input. -/
structure Valid (i : PricingInput) : Prop where
  lines : ∀ l ∈ i.cart.lines, LineOk i.cart.currency l
  line_ids : (i.cart.lines.map (·.line_id)).Nodup
  order_cap_amount : ∀ m, i.policy.order_cap.amount = some m →
    m.currency = i.cart.currency ∧ 0 ≤ m.amount ∧ m.amount ≤ MAX_MINOR_UNITS
  order_cap_bps : ∀ b, i.policy.order_cap.bps = some b → 0 ≤ b ∧ b ≤ BPS_SCALE
  group_ids : (i.groups.map (·.group_id)).Nodup
  promotions : ∀ p ∈ i.promotions, PromotionOk i.cart.currency (i.groups.map (·.group_id)) p
  promotion_ids : (i.promotions.map (·.promotion_id)).Nodup

theorem validate_ok {i : PricingInput} (h : validate i = .ok ()) : Valid i := by
  unfold validate at h
  obtain ⟨_, h2⟩ := andThen_ok h
  obtain ⟨hl, h3⟩ := andThen_ok h2
  obtain ⟨hld, h4⟩ := andThen_ok h3
  obtain ⟨_, h5⟩ := andThen_ok h4
  obtain ⟨hoc, h6⟩ := andThen_ok h5
  obtain ⟨hgd, h7⟩ := andThen_ok h6
  obtain ⟨hp, hpd⟩ := andThen_ok h7
  have hoc' := checkOrderCap_ok hoc
  exact { lines := checkLines_ok hl
          line_ids := checkDuplicates_ok hld
          order_cap_amount := hoc'.1
          order_cap_bps := hoc'.2
          group_ids := checkDuplicates_ok hgd
          promotions := checkPromotions_ok hp
          promotion_ids := checkDuplicates_ok hpd }

end Discount
