//! Differential random testing: `discount_core::apply` against the Lean reference on
//! random inputs (ADR 2). One reference process serves the whole run.

use std::sync::{Mutex, OnceLock};

use discount_core::{PricingInput, apply};
use discount_drt::generators::{input, shuffle, valid_input};
use discount_drt::{Reference, cases, rust_evaluate};
use proptest::prelude::*;

fn reference() -> &'static Mutex<Reference> {
    static REF: OnceLock<Mutex<Reference>> = OnceLock::new();
    REF.get_or_init(|| {
        Mutex::new(Reference::spawn().expect("spawn Lean reference (run `lake build` in spec/)"))
    })
}

fn check_against_reference(i: &PricingInput) -> Result<(), TestCaseError> {
    let lean = reference()
        .lock()
        .unwrap()
        .evaluate(i)
        .expect("reference answer");
    let rust = rust_evaluate(i);
    prop_assert_eq!(
        &rust,
        &lean,
        "\ninput:\n{}\nrust:\n{}\nlean:\n{}",
        serde_json::to_string_pretty(i).unwrap(),
        serde_json::to_string_pretty(&rust).unwrap(),
        serde_json::to_string_pretty(&lean).unwrap()
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), max_shrink_iters: 2_000, .. ProptestConfig::default() })]

    /// Core and reference agree on every generated input, valid or not.
    #[test]
    fn core_agrees_with_reference(i in input()) {
        check_against_reference(&i)?;
    }

    /// Invariant 3, permutation part, on the Rust side: the order in which promotions and
    /// groups are listed does not change the result of a valid input.
    #[test]
    fn permutation_invariance(i in valid_input(), seed in any::<u64>()) {
        let mut j = i.clone();
        shuffle(&mut j.promotions, seed);
        shuffle(&mut j.groups, seed.rotate_left(17));
        let a = apply(&i);
        let b = apply(&j);
        if a.is_ok() {
            prop_assert_eq!(a, b);
        }
    }
}
