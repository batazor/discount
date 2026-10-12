//! `eligible` (ADR 7). Mirrors `spec/Discount/Eligibility.lean`.

use crate::types::*;

pub fn rule_holds(r: &Rule, i: &PricingInput) -> bool {
    match r {
        Rule::DateWindow { starts_at, ends_at } => {
            let after_start = match starts_at {
                None => true,
                Some(s) => *s <= i.now,
            };
            let before_end = match ends_at {
                None => true,
                Some(e) => i.now < *e,
            };
            after_start && before_end
        }
    }
}

/// Index of the first rule that does not hold, for `SkipReason::NotEligible`.
pub fn first_failing_rule(rules: &[Rule], i: &PricingInput) -> Option<usize> {
    for (k, r) in rules.iter().enumerate() {
        if !rule_holds(r, i) {
            return Some(k);
        }
    }
    None
}

pub fn eligible(p: &Promotion, i: &PricingInput) -> bool {
    first_failing_rule(&p.rules, i).is_none()
}
