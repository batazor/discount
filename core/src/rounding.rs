//! `roundDiv` (ADR 3). `q = num / den` and `r = num % den` with Euclidean semantics, which
//! is floor for the non-negative `num` used everywhere in the engine. Mirrors
//! `spec/Discount/Rounding.lean`.

use crate::types::RoundingPolicy;

/// Kept inside the Aeneas-friendly subset: safe, no closures, no traits. The branch
/// structure mirrors the Lean definition case for case.
#[allow(clippy::if_same_then_else)]
pub fn round_div(p: RoundingPolicy, num: i64, den: i64) -> i64 {
    let q = num.div_euclid(den);
    let r = num.rem_euclid(den);
    match p {
        RoundingPolicy::Down => q,
        RoundingPolicy::HalfUp => {
            if 2 * r >= den {
                q + 1
            } else {
                q
            }
        }
        RoundingPolicy::HalfEven => {
            if 2 * r > den {
                q + 1
            } else if 2 * r < den {
                q
            } else if q.rem_euclid(2) == 0 {
                q
            } else {
                q + 1
            }
        }
    }
}
