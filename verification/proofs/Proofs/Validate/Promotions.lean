import Proofs.Validate.Checks

/-!
# What the promotion checks guarantee (ADR 6): `PromotionOk` on the normalized promotion
-/
namespace Discount

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

end Discount
