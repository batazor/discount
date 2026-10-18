import Proofs.Compute
import Proofs.Invariants.Eligibility

/-!
# Invariant 3: stacking is deterministic (permutation invariance)

For any permutations `π` of the promotions and `σ` of the groups,
`apply { input with promotions := π, groups := σ } = apply input` on the `ok` branch, and
`validate` accepts one iff it accepts the other. The caller's list orders never matter:
`sortPromotions` is a `mergeSort` under a total, transitive order whose last key is the
unique `promotion_id`, so the sorted list is the same for every permutation; everything
after sorting reads the groups only through `find?` by the unique `group_id`.
-/
namespace Discount

theorem string_lt_of_ne_of_not_lt {a b : String} (hne : a ≠ b) (h : ¬ b < a) : a < b := by
  by_cases h2 : a < b
  · exact h2
  · exact absurd (String.le_antisymm (String.not_lt.mp h) (String.not_lt.mp h2)) hne

/-! ## The total order behind `sortPromotions` -/

/-- `sortLe` as a proposition: lexicographic on
`(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)`. -/
def LexLe (groups : List StackingGroup) (a b : Promotion) : Prop :=
  groupPriority groups b.group_id < groupPriority groups a.group_id ∨
  (groupPriority groups a.group_id = groupPriority groups b.group_id ∧
    (a.group_id < b.group_id ∨
    (a.group_id = b.group_id ∧
      (b.priority < a.priority ∨
      (a.priority = b.priority ∧ ¬ b.promotion_id < a.promotion_id)))))

theorem sortLe_iff (groups : List StackingGroup) (a b : Promotion) :
    sortLe groups a b = true ↔ LexLe groups a b := by
  unfold sortLe LexLe
  simp only [bne_iff_ne, ne_eq]
  split
  · rename_i h; simp [h]
  · rename_i h1
    have h1' := Decidable.of_not_not h1
    split
    · rename_i h2; simp [h1', h2]
    · rename_i h2
      have h2' := Decidable.of_not_not h2
      split
      · rename_i h3; simp [h2', h3]
      · rename_i h3
        have h3' := Decidable.of_not_not h3
        simp [h2', h3']

theorem lexLe_total (groups : List StackingGroup) (a b : Promotion) :
    LexLe groups a b ∨ LexLe groups b a := by
  unfold LexLe
  rcases Int.lt_trichotomy (groupPriority groups a.group_id) (groupPriority groups b.group_id)
    with h | h | h
  · right; left; exact h
  · by_cases hg : a.group_id = b.group_id
    · rcases Int.lt_trichotomy a.priority b.priority with hp | hp | hp
      · right; right; exact ⟨h.symm, Or.inr ⟨hg.symm, Or.inl hp⟩⟩
      · by_cases hid : b.promotion_id < a.promotion_id
        · right; right; exact ⟨h.symm, Or.inr ⟨hg.symm, Or.inr ⟨hp.symm, String.lt_asymm hid⟩⟩⟩
        · left; right; exact ⟨h, Or.inr ⟨hg, Or.inr ⟨hp, hid⟩⟩⟩
      · left; right; exact ⟨h, Or.inr ⟨hg, Or.inl hp⟩⟩
    · by_cases hlt : a.group_id < b.group_id
      · left; right; exact ⟨h, Or.inl hlt⟩
      · right; right; exact ⟨h.symm, Or.inl (string_lt_of_ne_of_not_lt (Ne.symm hg) hlt)⟩
  · left; left; exact h

theorem lexLe_trans (groups : List StackingGroup) (a b c : Promotion)
    (h1 : LexLe groups a b) (h2 : LexLe groups b c) : LexLe groups a c := by
  unfold LexLe at *
  rcases h1 with h1 | ⟨g1, h1⟩
  · rcases h2 with h2 | ⟨g2, _⟩
    · left; omega
    · left; omega
  · rcases h2 with h2 | ⟨g2, h2⟩
    · left; omega
    · right
      refine ⟨by omega, ?_⟩
      rcases h1 with h1 | ⟨e1, h1⟩
      · rcases h2 with h2 | ⟨e2, _⟩
        · left; exact String.lt_trans h1 h2
        · left; rw [← e2]; exact h1
      · rcases h2 with h2 | ⟨e2, h2⟩
        · left; rw [e1]; exact h2
        · right
          refine ⟨e1.trans e2, ?_⟩
          rcases h1 with h1 | ⟨p1, h1⟩
          · rcases h2 with h2 | ⟨p2, _⟩
            · left; omega
            · left; omega
          · rcases h2 with h2 | ⟨p2, h2⟩
            · left; omega
            · right
              refine ⟨p1.trans p2, ?_⟩
              intro hca
              -- ¬ b < a, ¬ c < b, c < a: then a ≤ b ≤ c < a
              have hab := String.not_lt.mp h1
              have hbc := String.not_lt.mp h2
              have : a.promotion_id ≤ c.promotion_id := String.le_trans hab hbc
              exact absurd hca (String.not_lt.mpr this)

theorem lexLe_antisymm (groups : List StackingGroup) (a b : Promotion)
    (h1 : LexLe groups a b) (h2 : LexLe groups b a) : a.promotion_id = b.promotion_id := by
  unfold LexLe at *
  rcases h1 with h1 | ⟨g1, h1⟩ <;> rcases h2 with h2 | ⟨g2, h2⟩
  · omega
  · omega
  · omega
  · rcases h1 with h1 | ⟨e1, h1⟩ <;> rcases h2 with h2 | ⟨e2, h2⟩
    · exact absurd h2 (String.lt_asymm h1)
    · exact absurd h1 (by rw [e2]; exact String.lt_irrefl _)
    · exact absurd h2 (by rw [e1]; exact String.lt_irrefl _)
    · rcases h1 with h1 | ⟨p1, h1⟩ <;> rcases h2 with h2 | ⟨p2, h2⟩
      · omega
      · omega
      · omega
      · exact String.le_antisymm (String.not_lt.mp h1) (String.not_lt.mp h2)

theorem sortLe_total (groups : List StackingGroup) (a b : Promotion) :
    (sortLe groups a b || sortLe groups b a) = true := by
  rcases lexLe_total groups a b with h | h
  · simp [(sortLe_iff groups a b).mpr h]
  · simp [(sortLe_iff groups b a).mpr h]

theorem sortLe_trans (groups : List StackingGroup) (a b c : Promotion)
    (h1 : sortLe groups a b = true) (h2 : sortLe groups b c = true) : sortLe groups a c = true :=
  (sortLe_iff groups a c).mpr
    (lexLe_trans groups a b c ((sortLe_iff groups a b).mp h1) ((sortLe_iff groups b c).mp h2))

/-- Sorting a permutation of a list with unique ids gives the same list. -/
theorem sortPromotions_perm (groups : List StackingGroup) {ps qs : List Promotion}
    (hp : ps.Perm qs) (hn : (ps.map (·.promotion_id)).Nodup) :
    sortPromotions groups ps = sortPromotions groups qs := by
  unfold sortPromotions
  apply eq_of_perm_of_pairwise (sortLe groups)
  · exact ((List.mergeSort_perm ps _).trans hp).trans (List.mergeSort_perm qs _).symm
  · exact List.pairwise_mergeSort (sortLe_trans groups) (sortLe_total groups) ps
  · exact List.pairwise_mergeSort (sortLe_trans groups) (sortLe_total groups) qs
  · intro a ha b hb h1 h2
    have hid := lexLe_antisymm groups a b ((sortLe_iff groups a b).mp h1) ((sortLe_iff groups b a).mp h2)
    exact eq_of_nodup_map hn (List.mem_mergeSort.mp ha) (List.mem_mergeSort.mp hb) hid

/-! ## The groups are read only through `find?` by the unique `group_id` -/

theorem findGroup_perm {g₁ g₂ : List StackingGroup} (hp : g₁.Perm g₂)
    (hn : (g₁.map (·.group_id)).Nodup) (gid : GroupId) : findGroup g₁ gid = findGroup g₂ gid := by
  unfold findGroup
  exact find?_key_eq_of_perm (·.group_id) hp hn gid

theorem groupPriority_perm {g₁ g₂ : List StackingGroup} (hp : g₁.Perm g₂)
    (hn : (g₁.map (·.group_id)).Nodup) : groupPriority g₁ = groupPriority g₂ := by
  funext gid
  unfold groupPriority
  rw [findGroup_perm hp hn]

theorem modeOf_perm {g₁ g₂ : List StackingGroup} (hp : g₁.Perm g₂)
    (hn : (g₁.map (·.group_id)).Nodup) : modeOf g₁ = modeOf g₂ := by
  funext gid
  unfold modeOf
  rw [findGroup_perm hp hn]

theorem sortPromotions_groups_perm {g₁ g₂ : List StackingGroup} (hp : g₁.Perm g₂)
    (hn : (g₁.map (·.group_id)).Nodup) (ps : List Promotion) :
    sortPromotions g₁ ps = sortPromotions g₂ ps := by
  unfold sortPromotions
  have : sortLe g₁ = sortLe g₂ := by
    funext a b
    unfold sortLe
    rw [groupPriority_perm hp hn]
  rw [this]

theorem selectFirstMatch_congr {g₁ g₂ : List StackingGroup} (h : modeOf g₁ = modeOf g₂) :
    ∀ (ps : List Promotion) (taken : List (GroupId × PromotionId)),
      selectFirstMatch g₁ ps taken = selectFirstMatch g₂ ps taken
  | [], _ => rfl
  | p :: ps, taken => by
      simp only [selectFirstMatch, h]
      split
      · rw [selectFirstMatch_congr h ps taken]
      · split
        · rw [selectFirstMatch_congr h ps taken]
        · rw [selectFirstMatch_congr h ps _]

/-! ## `validate` does not depend on the order -/

theorem andThen_ok_iff {a b : Except PricingError Unit} :
    andThen a b = .ok () ↔ a = .ok () ∧ b = .ok () := by
  constructor
  · exact andThen_ok
  · rintro ⟨rfl, rfl⟩; rfl

theorem firstDuplicate_eq_none {l seen : List String} (hn : l.Nodup) (hs : ∀ x ∈ l, x ∉ seen) :
    firstDuplicate l seen = none := by
  induction l generalizing seen with
  | nil => rfl
  | cons x xs ih =>
    simp only [firstDuplicate]
    have hx : x ∉ seen := hs x (by simp)
    simp only [hx, ↓reduceIte]
    apply ih (List.nodup_cons.mp hn).2
    intro y hy hmem
    rcases List.mem_cons.mp hmem with rfl | hmem'
    · exact (List.nodup_cons.mp hn).1 hy
    · exact hs y (by simp [hy]) hmem'

theorem checkDuplicates_eq_ok_iff {ids : List String} {mk : String → PricingError} :
    checkDuplicates ids mk = .ok () ↔ ids.Nodup := by
  constructor
  · exact checkDuplicates_ok
  · intro hn
    unfold checkDuplicates
    rw [firstDuplicate_eq_none hn (fun _ _ h => by simp at h)]

/-- The index-free content of `checkMoney`. -/
def moneyOk (m : Money) (cur : Currency) (rates : List ExchangeRate) : Bool :=
  validCurrency m.currency &&
    match convertAmount rates m.amount m.currency cur with
    | none => false
    | some c => amountInRange m.amount && amountInRange c

theorem checkMoney_eq_ok_iff {m : Money} {cur : Currency} {rates : List ExchangeRate} {f : String} :
    checkMoney m cur rates f = .ok () ↔ moneyOk m cur rates = true := by
  unfold checkMoney moneyOk
  split
  · simp_all
  · split
    · simp_all
    · split
      · simp_all
      · split <;> simp_all

def optMoneyOk (m : Option Money) (cur : Currency) (rates : List ExchangeRate) : Bool :=
  match m with
  | none => true
  | some m => moneyOk m cur rates

theorem checkOptMoney_eq_ok_iff {m : Option Money} {cur : Currency} {rates : List ExchangeRate}
    {f : String} : checkOptMoney m cur rates f = .ok () ↔ optMoneyOk m cur rates = true := by
  cases m <;> simp [checkOptMoney, optMoneyOk, checkMoney_eq_ok_iff]

def actionOk (a : Action) (cur : Currency) (rates : List ExchangeRate) : Bool :=
  match a with
  | .OrderPercentage b => bpsInRange b
  | .OrderFixedAmount m => moneyOk m cur rates

theorem checkAction_eq_ok_iff {a : Action} {cur : Currency} {rates : List ExchangeRate} {i : Nat} :
    checkAction a cur rates i = .ok () ↔ actionOk a cur rates = true := by
  cases a with
  | OrderPercentage b =>
    simp only [checkAction, actionOk]
    split <;> simp_all
  | OrderFixedAmount m => simp [checkAction, actionOk, checkMoney_eq_ok_iff]

def ruleOk (r : Rule) (cur : Currency) (rates : List ExchangeRate) : Bool :=
  match r with
  | .OrderTotalRange mn mx => optMoneyOk mn cur rates && optMoneyOk mx cur rates
  | .MaxCompletedOrders m => decide (0 ≤ m)
  | .MaxUsesPerCustomer m => decide (0 ≤ m)
  | .MaxUsesTotal m => decide (0 ≤ m)
  | _ => true

theorem checkRule_eq_ok_iff {r : Rule} {cur : Currency} {rates : List ExchangeRate} {i k : Nat} :
    checkRule r cur rates i k = .ok () ↔ ruleOk r cur rates = true := by
  cases r <;> simp only [checkRule, ruleOk, andThen_ok_iff, checkOptMoney_eq_ok_iff]
  all_goals first
    | simp
    | (split <;> simp_all <;> omega)

theorem checkRules_eq_ok_iff {rs : List Rule} {cur : Currency} {rates : List ExchangeRate} {i k : Nat} :
    checkRules rs cur rates i k = .ok () ↔ rs.all (fun r => ruleOk r cur rates) = true := by
  induction rs generalizing k with
  | nil => simp [checkRules]
  | cons r rs ih =>
    simp only [checkRules, andThen_ok_iff, checkRule_eq_ok_iff, ih, List.all_cons, Bool.and_eq_true]

/-- The index-free content of one promotion's checks. -/
def promotionOk (p : Promotion) (cur : Currency) (rates : List ExchangeRate) (gids : List GroupId) : Bool :=
  actionOk p.action cur rates && optMoneyOk p.cap cur rates && optMoneyOk p.remaining_budget cur rates &&
    decide (0 ≤ p.uses_total) && p.rules.all (fun r => ruleOk r cur rates) && decide (p.group_id ∈ gids)

theorem checkPromotions_eq_ok_iff {ps : List Promotion} {cur : Currency} {rates : List ExchangeRate}
    {gids : List GroupId} {k : Nat} :
    checkPromotions ps cur rates gids k = .ok () ↔ ∀ p ∈ ps, promotionOk p cur rates gids = true := by
  induction ps generalizing k with
  | nil => simp [checkPromotions]
  | cons p ps ih =>
    simp only [checkPromotions, andThen_ok_iff, checkAction_eq_ok_iff, checkOptMoney_eq_ok_iff,
      checkRules_eq_ok_iff, ih, List.mem_cons, forall_eq_or_imp, promotionOk, Bool.and_eq_true,
      decide_eq_true_eq]
    constructor
    · rintro ⟨h1, h2, h3, h4, h5, h6, h7⟩
      refine ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, ?_⟩, h5⟩, ?_⟩, h7⟩
      · split at h4
        · exact nomatch h4
        · omega
      · split at h6
        · assumption
        · exact nomatch h6
    · rintro ⟨⟨⟨⟨⟨⟨h1, h2⟩, h3⟩, h4⟩, h5⟩, h6⟩, h7⟩
      refine ⟨h1, h2, h3, ?_, h5, ?_, h7⟩
      · split
        · exfalso; omega
        · rfl
      · split
        · rfl
        · rename_i hneg; exact absurd h6 hneg

theorem promotionOk_gids_congr {p : Promotion} {cur : Currency} {rates : List ExchangeRate}
    {g₁ g₂ : List GroupId} (h : ∀ x, x ∈ g₁ ↔ x ∈ g₂) :
    promotionOk p cur rates g₁ = promotionOk p cur rates g₂ := by
  simp only [promotionOk, h]

/-- `validate` accepts a permuted input whenever it accepts the original. -/
theorem validate_perm {i : PricingInput} {π : List Promotion} {σ : List StackingGroup}
    (hπ : π.Perm i.promotions) (hσ : σ.Perm i.groups) (h : validate i = .ok ()) :
    validate { i with promotions := π, groups := σ } = .ok () := by
  unfold validate at h ⊢
  simp only [andThen_ok_iff] at h ⊢
  obtain ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9, h10, h11, h12⟩ := h
  refine ⟨h1, h2, h3, h4, h5, h6, h7, h8, h9, ?_, ?_, ?_⟩
  · rw [checkDuplicates_eq_ok_iff] at h10 ⊢
    exact (hσ.map _).nodup_iff.mpr h10
  · rw [checkPromotions_eq_ok_iff] at h11 ⊢
    intro p hp
    rw [promotionOk_gids_congr (fun x => (hσ.map (·.group_id)).mem_iff)]
    exact h11 p (hπ.mem_iff.mp hp)
  · rw [checkDuplicates_eq_ok_iff] at h12 ⊢
    exact (hπ.map _).nodup_iff.mpr h12

/-! ## `compute` reads the promotions only through `sortedOf` and the groups only through `modeOf` -/

section Congr

variable {i j : PricingInput} (hcart : i.cart = j.cart) (hnow : i.now = j.now)
  (hcust : i.customer = j.customer) (hcoup : i.coupons = j.coupons) (hpol : i.policy = j.policy)
  (hsorted : sortedOf i = sortedOf j) (hmode : modeOf i.groups = modeOf j.groups)

include hcart hnow hcust hcoup in
theorem eligibleOf_congr (hsorted : sortedOf i = sortedOf j) : eligibleOf i = eligibleOf j := by
  unfold eligibleOf
  rw [hsorted]
  apply List.filter_congr
  intro p _
  exact eligible_depends_on_input p i j hnow hcart hcust hcoup

include hcart hnow hcust hcoup in
theorem skipsOf_congr (ps : List Promotion) : skipsOf i ps = skipsOf j ps := by
  unfold skipsOf
  congr 1
  funext p
  rw [firstFailingRule_congr p.rules p i j hnow hcart hcust hcoup]

include hcart hnow hcust hcoup hsorted in
theorem afterExclusive_congr : afterExclusive i = afterExclusive j := by
  unfold afterExclusive
  rw [eligibleOf_congr hcart hnow hcust hcoup hsorted]

include hcart hnow hcust hcoup hsorted hmode in
theorem afterFirstMatch_congr : afterFirstMatch i = afterFirstMatch j := by
  unfold afterFirstMatch
  rw [afterExclusive_congr hcart hnow hcust hcoup hsorted, selectFirstMatch_congr hmode]

include hcart in
theorem subsOf_congr : subsOf i = subsOf j := by
  unfold subsOf
  rw [hcart]

include hcart hpol in
theorem applyOne_congr : applyOne i = applyOne j := by
  funext S st p
  simp only [applyOne, hcart, hpol]

include hcart hpol in
theorem initStep_congr : initStep i = initStep j := by
  unfold initStep
  rw [subsOf_congr hcart, hpol]

include hcart hnow hcust hcoup hpol hsorted hmode in
theorem finalStep_congr : finalStep i = finalStep j := by
  unfold finalStep toApplyOf
  rw [afterFirstMatch_congr hcart hnow hcust hcoup hsorted hmode, applyOne_congr hcart hpol,
    subsOf_congr hcart, initStep_congr hcart hpol]

include hcart hnow hcust hcoup hpol hsorted hmode in
theorem compute_congr : compute i = compute j := by
  unfold compute
  simp only [subsOf_congr hcart, hcart, hsorted, skipsOf_congr hcart hnow hcust hcoup,
    afterExclusive_congr hcart hnow hcust hcoup hsorted,
    afterFirstMatch_congr hcart hnow hcust hcoup hsorted hmode,
    finalStep_congr hcart hnow hcust hcoup hpol hsorted hmode]

end Congr

/-! ## The invariant -/

/-- Invariant 3, permutation part: the result does not depend on the order of the
promotions or of the groups in the input. -/
theorem apply_perm_invariant {i : PricingInput} {π : List Promotion} {σ : List StackingGroup}
    (hπ : π.Perm i.promotions) (hσ : σ.Perm i.groups) {r : PricingResult} (h : apply i = .ok r) :
    apply { i with promotions := π, groups := σ } = .ok r := by
  obtain ⟨hval, rfl⟩ := apply_ok h
  have hv := validate_ok hval
  have hval' := validate_perm hπ hσ hval
  unfold apply
  rw [hval']
  have hgn : (i.groups.map (·.group_id)).Nodup := hv.group_ids
  have hpn : ((normalize i).promotions.map (·.promotion_id)).Nodup := hv.promotion_ids
  rw [normalize_promotions] at hpn
  let j : PricingInput := normalize { i with promotions := π, groups := σ }
  have hmode : modeOf j.groups = modeOf (normalize i).groups :=
    modeOf_perm hσ ((hσ.map _).nodup_iff.mpr hgn)
  have hsorted : sortedOf j = sortedOf (normalize i) := by
    unfold sortedOf
    show sortPromotions σ (π.map (normalizePromotion i.exchange_rates i.cart.currency)) =
      sortPromotions i.groups (i.promotions.map (normalizePromotion i.exchange_rates i.cart.currency))
    rw [sortPromotions_groups_perm hσ ((hσ.map _).nodup_iff.mpr hgn)]
    have hperm := hπ.map (normalizePromotion i.exchange_rates i.cart.currency)
    exact sortPromotions_perm i.groups hperm ((hperm.map _).nodup_iff.mpr hpn)
  exact congrArg Except.ok (compute_congr (i := j) (j := normalize i) rfl rfl rfl rfl rfl hsorted hmode)

end Discount
