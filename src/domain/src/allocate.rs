//! `allocate` (ADR 5): largest-remainder allocation of an order-level discount `D` over
//! line subtotals `s` (`S = Σ s`). Mirrors `spec/Discount/Allocate.lean` piece by piece.
//! `D * s_i` can reach `10^28`, so the products live in `i128`; every result fits `i64`.
//! Kept inside the Aeneas-friendly subset: safe Rust, loops, no trait objects.

/// Indices of the positive entries, in order.
pub fn positive_indices(rems: &[i64]) -> Vec<usize> {
    let mut out = Vec::new();
    for (k, r) in rems.iter().enumerate() {
        if 0 < *r {
            out.push(k);
        }
    }
    out
}

/// `r_j < r_i`, or equal remainders and `i` comes first: largest remainder, ties by position.
pub fn remainder_le(rems: &[i64], i: usize, j: usize) -> bool {
    rems[j] < rems[i] || (rems[i] == rems[j] && i <= j)
}

/// Add one minor unit to every picked line.
pub fn bump_all(base: &[i64], picked: &[usize]) -> Vec<i64> {
    let mut acc = base.to_vec();
    for &j in picked {
        acc[j] += 1;
    }
    acc
}

/// `floor(D * s_i / S)` per line.
pub fn base_shares(d: i64, s_sum: i64, s: &[i64]) -> Vec<i64> {
    s.iter()
        .map(|&si| (((d as i128) * (si as i128)).div_euclid(s_sum as i128)) as i64)
        .collect()
}

/// `(D * s_i) mod S` per line.
pub fn remainders(d: i64, s_sum: i64, s: &[i64]) -> Vec<i64> {
    s.iter()
        .map(|&si| (((d as i128) * (si as i128)).rem_euclid(s_sum as i128)) as i64)
        .collect()
}

/// `R = D - Σ base_i`, the minor units still to distribute.
pub fn extra_units(d: i64, s_sum: i64, s: &[i64]) -> i64 {
    d - base_shares(d, s_sum, s).iter().sum::<i64>()
}

/// The `R` candidate lines with the largest remainders, ties by position.
pub fn picked_lines(d: i64, s_sum: i64, s: &[i64]) -> Vec<usize> {
    let rems = remainders(d, s_sum, s);
    let mut sorted = positive_indices(&rems);
    // Same order as a stable merge sort by `remainder_le`: candidates are distinct indices,
    // so the comparator below is a strict total order with no ties.
    sorted.sort_by(|&i, &j| rems[j].cmp(&rems[i]).then(i.cmp(&j)));
    let r = extra_units(d, s_sum, s);
    let take = if r < 0 {
        0
    } else {
        (r as usize).min(sorted.len())
    };
    sorted.truncate(take);
    sorted
}

pub fn allocate(d: i64, s: &[i64]) -> Vec<i64> {
    let s_sum: i64 = s.iter().sum();
    if s_sum <= 0 {
        vec![0; s.len()]
    } else {
        bump_all(&base_shares(d, s_sum, s), &picked_lines(d, s_sum, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserves_and_bounds() {
        let s = [3998, 501];
        let a = allocate(449, &s);
        assert_eq!(a, vec![399, 50]);
        assert_eq!(a.iter().sum::<i64>(), 449);
        let a = allocate(300, &[3599, 451]);
        assert_eq!(a, vec![267, 33]);
    }

    #[test]
    fn zero_total() {
        assert_eq!(allocate(0, &[0, 0]), vec![0, 0]);
        assert_eq!(allocate(0, &[]), Vec::<i64>::new());
    }

    #[test]
    fn full_discount_takes_everything() {
        assert_eq!(allocate(10, &[3, 7]), vec![3, 7]);
    }
}
