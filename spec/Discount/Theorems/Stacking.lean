import Discount.Stacking

/-!
# Exclusivity and first-match lemmas (ADR 4, invariant 4)
-/
namespace Discount

theorem selectExclusive_mem {ps : List Promotion} {q : Promotion} (h : q ∈ (selectExclusive ps).1) :
    q ∈ ps := by
  unfold selectExclusive at h
  split at h
  · rename_i w hw
    simp at h
    subst h
    exact List.mem_of_find?_eq_some hw
  · exact h

theorem selectExclusive_exclusive {ps : List Promotion} {q : Promotion}
    (hq : q ∈ (selectExclusive ps).1) (hex : q.exclusive = true) : (selectExclusive ps).1 = [q] := by
  unfold selectExclusive at hq ⊢
  split at hq
  · rename_i w hw
    simp at hq
    subst hq
    simp
  · rename_i hnone
    exfalso
    exact (List.find?_eq_none.mp hnone) q hq hex

theorem selectFirstMatch_sublist (groups : List StackingGroup) (ps : List Promotion)
    (taken : List (GroupId × PromotionId)) : (selectFirstMatch groups ps taken).1.Sublist ps := by
  induction ps generalizing taken with
  | nil => simp [selectFirstMatch]
  | cons p ps ih =>
    simp only [selectFirstMatch]
    split
    · exact (ih taken).cons_cons p
    · split
      · exact (ih taken).cons p
      · exact (ih _).cons_cons p

theorem selectFirstMatch_mem {groups : List StackingGroup} {ps : List Promotion}
    {taken : List (GroupId × PromotionId)} {q : Promotion}
    (h : q ∈ (selectFirstMatch groups ps taken).1) : q ∈ ps :=
  (selectFirstMatch_sublist groups ps taken).subset h

theorem selectFirstMatch_length_le (groups : List StackingGroup) (ps : List Promotion)
    (taken : List (GroupId × PromotionId)) :
    (selectFirstMatch groups ps taken).1.length ≤ ps.length :=
  (selectFirstMatch_sublist groups ps taken).length_le

/-- In a `FirstMatch` group, the kept list holds at most one promotion of that group, and
none at all if the group is already taken. -/
theorem selectFirstMatch_at_most_one (groups : List StackingGroup) (gid : GroupId)
    (hmode : modeOf groups gid = .FirstMatch) (ps : List Promotion)
    (taken : List (GroupId × PromotionId)) :
    ((selectFirstMatch groups ps taken).1.filter (fun p => p.group_id == gid)).length ≤
      (if (taken.lookup gid).isSome then 0 else 1) := by
  induction ps generalizing taken with
  | nil => simp [selectFirstMatch]
  | cons p ps ih =>
    simp only [selectFirstMatch]
    split
    · -- Stackable: p is not in group gid
      rename_i hst
      have hne : p.group_id ≠ gid := by
        intro heq; rw [heq, hmode] at hst; exact nomatch hst
      simp only [List.filter_cons, beq_iff_eq, hne, ite_false]
      exact ih taken
    · split
      · -- FirstMatch, already taken: p dropped
        exact ih taken
      · -- FirstMatch, first of its group: p kept
        rename_i hlookup
        by_cases heq : p.group_id = gid
        · subst heq
          simp only [List.filter_cons, beq_self_eq_true, ite_true, List.length_cons]
          have := ih ((p.group_id, p.promotion_id) :: taken)
          simp [List.lookup, hlookup] at this ⊢
          omega
        · simp only [List.filter_cons, beq_iff_eq, heq, ite_false]
          have := ih ((p.group_id, p.promotion_id) :: taken)
          have hl : ((p.group_id, p.promotion_id) :: taken).lookup gid = taken.lookup gid := by
            have hb : (gid == p.group_id) = false := beq_eq_false_iff_ne.mpr (Ne.symm heq)
            simp [List.lookup, hb]
          rw [hl] at this
          exact this

theorem selectFirstMatch_at_most_one_nil (groups : List StackingGroup) (gid : GroupId)
    (hmode : modeOf groups gid = .FirstMatch) (ps : List Promotion) :
    ((selectFirstMatch groups ps []).1.filter (fun p => p.group_id == gid)).length ≤ 1 := by
  have := selectFirstMatch_at_most_one groups gid hmode ps []
  simpa [List.lookup] using this

end Discount
