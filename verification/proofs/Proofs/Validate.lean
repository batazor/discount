import Proofs.Validate.Promotions

/-!
# What `validate = ok` guarantees (ADR 6)

`validate_ok` is the bridge from the sequential checks to the facts the arithmetic proofs
need, stated on the **normalized** input (`normalize`): every amount the pipeline sees is in
the cart currency and within bounds.
-/
namespace Discount

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
