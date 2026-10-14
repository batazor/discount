//! Total order, exclusivity and first-match groups (ADR 4). Mirrors
//! `spec/Discount/Stacking.lean`.

use std::cmp::Ordering;

use crate::types::*;

pub fn find_group<'a>(groups: &'a [StackingGroup], gid: &str) -> Option<&'a StackingGroup> {
    groups.iter().find(|g| g.group_id == gid)
}

pub fn group_priority(groups: &[StackingGroup], gid: &str) -> i64 {
    match find_group(groups, gid) {
        Some(g) => g.priority,
        None => 0,
    }
}

pub fn mode_of(groups: &[StackingGroup], gid: &str) -> StackingMode {
    match find_group(groups, gid) {
        Some(g) => g.mode,
        None => StackingMode::Stackable,
    }
}

/// `(group.priority DESC, group_id ASC, promotion.priority DESC, promotion_id ASC)` as a
/// total `le` relation, exactly as the Lean `sortLe`.
pub fn sort_le(groups: &[StackingGroup], a: &Promotion, b: &Promotion) -> bool {
    let ga = group_priority(groups, &a.group_id);
    let gb = group_priority(groups, &b.group_id);
    if ga != gb {
        gb < ga
    } else if a.group_id != b.group_id {
        a.group_id < b.group_id
    } else if a.priority != b.priority {
        b.priority < a.priority
    } else {
        !(b.promotion_id < a.promotion_id)
    }
}

/// Stable sort by `sort_le`. With unique promotion ids the relation is a total order, so
/// the result does not depend on the sorting algorithm; Lean uses a stable merge sort.
pub fn sort_promotions(groups: &[StackingGroup], ps: &[Promotion]) -> Vec<Promotion> {
    let mut out: Vec<Promotion> = ps.to_vec();
    out.sort_by(|a, b| {
        let ab = sort_le(groups, a, b);
        let ba = sort_le(groups, b, a);
        match (ab, ba) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            // A total relation never reaches this arm; treat as equal to stay stable.
            (false, false) => Ordering::Equal,
        }
    });
    out
}

/// The first exclusive promotion alone, or the whole list. Returns the kept promotions
/// and the skip reasons.
pub fn select_exclusive(ps: &[Promotion]) -> (Vec<Promotion>, Vec<(PromotionId, SkipReason)>) {
    match ps.iter().find(|p| p.exclusive) {
        Some(w) => {
            let skips = ps
                .iter()
                .filter(|p| p.promotion_id != w.promotion_id)
                .map(|p| {
                    (
                        p.promotion_id.clone(),
                        SkipReason::ExclusiveApplied {
                            winner: w.promotion_id.clone(),
                        },
                    )
                })
                .collect();
            (vec![w.clone()], skips)
        }
        None => (ps.to_vec(), Vec::new()),
    }
}

/// Per `FirstMatch` group keep the first promotion in the total order, skip the rest.
/// `taken` maps a group id to the winner's promotion id.
pub fn select_first_match(
    groups: &[StackingGroup],
    ps: &[Promotion],
    taken: &[(GroupId, PromotionId)],
) -> (Vec<Promotion>, Vec<(PromotionId, SkipReason)>) {
    let mut taken: Vec<(GroupId, PromotionId)> = taken.to_vec();
    let mut kept: Vec<Promotion> = Vec::new();
    let mut skips: Vec<(PromotionId, SkipReason)> = Vec::new();
    for p in ps {
        match mode_of(groups, &p.group_id) {
            StackingMode::Stackable => kept.push(p.clone()),
            StackingMode::FirstMatch => match lookup(&taken, &p.group_id) {
                Some(winner) => skips.push((
                    p.promotion_id.clone(),
                    SkipReason::FirstMatchTaken {
                        winner: winner.clone(),
                    },
                )),
                None => {
                    // Lean conses onto the front; with unique group ids the position is irrelevant.
                    taken.insert(0, (p.group_id.clone(), p.promotion_id.clone()));
                    kept.push(p.clone());
                }
            },
        }
    }
    (kept, skips)
}

/// `List.lookup`: the value of the first pair whose key matches.
pub fn lookup<'a, V>(pairs: &'a [(String, V)], key: &str) -> Option<&'a V> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}
