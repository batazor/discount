import Discount.Types

/-!
# Total order, exclusivity and first-match groups (ADR 4)
-/
namespace Discount

def findGroup (groups : List StackingGroup) (gid : GroupId) : Option StackingGroup :=
  groups.find? (fun g => g.group_id == gid)

def groupPriority (groups : List StackingGroup) (gid : GroupId) : Int :=
  match findGroup groups gid with
  | some g => g.priority
  | none => 0

def modeOf (groups : List StackingGroup) (gid : GroupId) : StackingMode :=
  match findGroup groups gid with
  | some g => g.mode
  | none => .Stackable

/-- `(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)` as a
total `le` relation for `mergeSort`. -/
def sortLe (groups : List StackingGroup) (a b : Promotion) : Bool :=
  let ga := groupPriority groups a.group_id
  let gb := groupPriority groups b.group_id
  if ga != gb then decide (gb < ga)
  else if a.group_id != b.group_id then decide (a.group_id < b.group_id)
  else if a.priority != b.priority then decide (b.priority < a.priority)
  else !decide (b.promotion_id < a.promotion_id)

def sortPromotions (groups : List StackingGroup) (ps : List Promotion) : List Promotion :=
  ps.mergeSort (sortLe groups)

/-- The first exclusive promotion alone, or the whole list. Returns the kept promotions
and the skip reasons. -/
def selectExclusive (ps : List Promotion) : List Promotion × List (PromotionId × SkipReason) :=
  match ps.find? (fun p => p.exclusive) with
  | some w =>
      ([w], (ps.filter (fun p => p.promotion_id != w.promotion_id)).map
              (fun p => (p.promotion_id, SkipReason.ExclusiveApplied w.promotion_id)))
  | none => (ps, [])

/-- Per `FirstMatch` group keep the first promotion in the total order, skip the rest.
`taken` maps a group id to the winner's promotion id. -/
def selectFirstMatch (groups : List StackingGroup) :
    List Promotion → List (GroupId × PromotionId) → List Promotion × List (PromotionId × SkipReason)
  | [], _ => ([], [])
  | p :: ps, taken =>
    match modeOf groups p.group_id with
    | .Stackable =>
        let r := selectFirstMatch groups ps taken
        (p :: r.1, r.2)
    | .FirstMatch =>
        match taken.lookup p.group_id with
        | some winner =>
            let r := selectFirstMatch groups ps taken
            (r.1, (p.promotion_id, SkipReason.FirstMatchTaken winner) :: r.2)
        | none =>
            let r := selectFirstMatch groups ps ((p.group_id, p.promotion_id) :: taken)
            (p :: r.1, r.2)

end Discount
