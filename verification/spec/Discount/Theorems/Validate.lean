import Discount.Validate
import Discount.Apply

/-!
# What `validate = ok` guarantees (ADR 6)

`validate_ok` is the bridge from the sequential checks to the facts the arithmetic proofs
need, stated on the **normalized** input (`normalize`): every amount the pipeline sees is
in the cart currency and within bounds.
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

/-- Facts about one validated, normalized promotion. -/
structure PromotionOk (cur : Currency) (gids : List GroupId) (p : Promotion) : Prop where
  percentage : ∀ b, p.action = .OrderPercentage b → 0 ≤ b ∧ b ≤ BPS_SCALE
  fixed : ∀ m, p.action = .OrderFixedAmount m → MoneyOk cur m
  cap : ∀ m, p.cap = some m → MoneyOk cur m
  budget : ∀ m, p.remaining_budget = some m → MoneyOk cur m
  group : p.group_id ∈ gids

theorem checkAction_ok {a : Action} {cur : Currency} {rates : List ExchangeRate} {i : Nat}
    (h : checkAction a cur rates i = .ok ()) :
    (∀ b, normalizeAction rates cur a = .OrderPercentage b → 0 ≤ b ∧ b ≤ BPS_SCALE) ∧
    (∀ m, normalizeAction rates cur a = .OrderFixedAmount m → MoneyOk cur m) := by
  unfold checkAction at h
  split at h
  · rename_i b
    split at h
    · exact nomatch h
    · rename_i hr
      refine ⟨fun b' hb => ?_, (fun m hm => nomatch hm)⟩
      simp [normalizeAction] at hb
      subst hb
      simpa [bpsInRange] using hr
  · rename_i m
    refine ⟨(fun b hb => nomatch hb), fun m' hm => ?_⟩
    simp [normalizeAction] at hm
    subst hm
    exact checkMoney_ok h

theorem normalizePromotion_cap (rates : List ExchangeRate) (cur : Currency) (p : Promotion) :
    (normalizePromotion rates cur p).cap = p.cap.map (normalizeMoney rates cur) := rfl
theorem normalizePromotion_budget (rates : List ExchangeRate) (cur : Currency) (p : Promotion) :
    (normalizePromotion rates cur p).remaining_budget = p.remaining_budget.map (normalizeMoney rates cur) := rfl
theorem normalizePromotion_action (rates : List ExchangeRate) (cur : Currency) (p : Promotion) :
    (normalizePromotion rates cur p).action = normalizeAction rates cur p.action := rfl
theorem normalizePromotion_group (rates : List ExchangeRate) (cur : Currency) (p : Promotion) :
    (normalizePromotion rates cur p).group_id = p.group_id := rfl
theorem normalizePromotion_id (rates : List ExchangeRate) (cur : Currency) (p : Promotion) :
    (normalizePromotion rates cur p).promotion_id = p.promotion_id := rfl

theorem checkPromotions_ok {ps : List Promotion} {cur : Currency} {rates : List ExchangeRate}
    {gids : List GroupId} {k : Nat} (h : checkPromotions ps cur rates gids k = .ok ()) :
    ∀ p ∈ ps, PromotionOk cur gids (normalizePromotion rates cur p) := by
  induction ps generalizing k with
  | nil => intro p hp; simp at hp
  | cons p ps ih =>
    intro x hx
    simp only [checkPromotions] at h
    obtain ⟨h1, h2⟩ := andThen_ok h
    obtain ⟨h3, h4⟩ := andThen_ok h2
    obtain ⟨h5, h6⟩ := andThen_ok h4
    obtain ⟨_, h7⟩ := andThen_ok h6
    obtain ⟨_, h8⟩ := andThen_ok h7
    obtain ⟨h9, h10⟩ := andThen_ok h8
    rcases List.mem_cons.mp hx with rfl | hx'
    · obtain ⟨ha1, ha2⟩ := checkAction_ok h1
      refine { percentage := ?_, fixed := ?_, cap := ?_, budget := ?_, group := ?_ }
      · intro b hb; exact ha1 b (by rwa [normalizePromotion_action] at hb)
      · intro m hm; exact ha2 m (by rwa [normalizePromotion_action] at hm)
      · intro m hm; exact checkOptMoney_ok h3 m (by rwa [normalizePromotion_cap] at hm)
      · intro m hm; exact checkOptMoney_ok h5 m (by rwa [normalizePromotion_budget] at hm)
      · rw [normalizePromotion_group]
        split at h9
        · assumption
        · exact nomatch h9
    · exact ih h10 x hx'

/-- Everything the proofs need from a validated input, stated on the normalized input. -/
structure Valid (i : PricingInput) : Prop where
  lines : ∀ l ∈ i.cart.lines, LineOk i.cart.currency l
  line_ids : (i.cart.lines.map (·.line_id)).Nodup
  order_cap_amount : ∀ m, i.policy.order_cap.amount = some m → MoneyOk i.cart.currency m
  order_cap_bps : ∀ b, i.policy.order_cap.bps = some b → 0 ≤ b ∧ b ≤ BPS_SCALE
  group_ids : (i.groups.map (·.group_id)).Nodup
  promotions : ∀ p ∈ i.promotions, PromotionOk i.cart.currency (i.groups.map (·.group_id)) p
  promotion_ids : (i.promotions.map (·.promotion_id)).Nodup

theorem normalize_cart (i : PricingInput) : (normalize i).cart = i.cart := rfl
theorem normalize_now (i : PricingInput) : (normalize i).now = i.now := rfl
theorem normalize_groups (i : PricingInput) : (normalize i).groups = i.groups := rfl
theorem normalize_customer (i : PricingInput) : (normalize i).customer = i.customer := rfl
theorem normalize_coupons (i : PricingInput) : (normalize i).coupons = i.coupons := rfl
theorem normalize_promotions (i : PricingInput) :
    (normalize i).promotions = i.promotions.map (normalizePromotion i.exchange_rates i.cart.currency) := rfl
theorem normalize_order_cap_amount (i : PricingInput) :
    (normalize i).policy.order_cap.amount =
      i.policy.order_cap.amount.map (normalizeMoney i.exchange_rates i.cart.currency) := rfl
theorem normalize_order_cap_bps (i : PricingInput) :
    (normalize i).policy.order_cap.bps = i.policy.order_cap.bps := rfl

theorem normalize_promotion_ids (i : PricingInput) :
    (normalize i).promotions.map (·.promotion_id) = i.promotions.map (·.promotion_id) := by
  rw [normalize_promotions, List.map_map]
  rfl

theorem validate_ok {i : PricingInput} (h : validate i = .ok ()) : Valid (normalize i) := by
  unfold validate at h
  obtain ⟨_, h2⟩ := andThen_ok h
  obtain ⟨hl, h3⟩ := andThen_ok h2
  obtain ⟨hld, h4⟩ := andThen_ok h3
  obtain ⟨_, h5⟩ := andThen_ok h4
  obtain ⟨_, h6⟩ := andThen_ok h5
  obtain ⟨_, h7⟩ := andThen_ok h6
  obtain ⟨_, h8⟩ := andThen_ok h7
  obtain ⟨_, h9⟩ := andThen_ok h8
  obtain ⟨hoc, h10⟩ := andThen_ok h9
  obtain ⟨hgd, h11⟩ := andThen_ok h10
  obtain ⟨hp, hpd⟩ := andThen_ok h11
  have hoc' := checkOrderCap_ok hoc
  refine { lines := ?_, line_ids := ?_, order_cap_amount := ?_, order_cap_bps := ?_, group_ids := ?_,
           promotions := ?_, promotion_ids := ?_ }
  · rw [normalize_cart]; exact checkLines_ok hl
  · rw [normalize_cart]; exact checkDuplicates_ok hld
  · rw [normalize_order_cap_amount, normalize_cart]; exact hoc'.1
  · rw [normalize_order_cap_bps]; exact hoc'.2
  · rw [normalize_groups]; exact checkDuplicates_ok hgd
  · intro p hp'
    rw [normalize_promotions, List.mem_map] at hp'
    obtain ⟨q, hq, rfl⟩ := hp'
    rw [normalize_cart, normalize_groups]
    exact checkPromotions_ok hp q hq
  · rw [normalize_promotion_ids]; exact checkDuplicates_ok hpd

end Discount
