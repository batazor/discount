//! Replays the hand-written fixtures through the Lean reference and the Rust core and
//! compares the parsed envelopes. Needs `lake build` in `spec/` first.

use discount_core::PricingInput;
use discount_drt::{Reference, rust_evaluate};
use serde_json::Value;

fn fixture_lines() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(dir).expect("fixtures dir") {
        let path = entry.expect("entry").path();
        let text = std::fs::read_to_string(&path).expect("read fixture");
        lines.extend(
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string),
        );
    }
    lines
}

#[test]
fn fixtures_agree_with_reference() {
    let mut reference =
        Reference::spawn().expect("spawn Lean reference (run `lake build` in spec/)");
    let lines = fixture_lines();
    assert!(!lines.is_empty());
    for line in lines {
        let lean = reference.evaluate_json(&line).expect("reference answer");
        match serde_json::from_str::<PricingInput>(&line) {
            Ok(input) => {
                let rust = rust_evaluate(&input);
                assert_eq!(rust, lean, "divergence on input {line}");
            }
            Err(_) => {
                assert!(
                    lean.get("decode_error").is_some(),
                    "Rust rejected what Lean decoded: {line} -> {lean}"
                );
            }
        }
    }
}

#[test]
fn unknown_key_is_rejected() {
    let bad = r#"{"amount": 1, "currency": "EUR", "extra": true}"#;
    assert!(serde_json::from_str::<discount_core::Money>(bad).is_err());
    let bad_enum = r#"{"type": "HalfUp", "extra": true}"#;
    assert!(serde_json::from_str::<discount_core::RoundingPolicy>(bad_enum).is_err());
    let v: Value = serde_json::to_value(discount_core::RoundingPolicy::HalfUp).unwrap();
    assert_eq!(v, serde_json::json!({"type": "HalfUp"}));
    let bad_action = r#"{"type": "OrderPercentage", "bps": 1, "extra": true}"#;
    assert!(serde_json::from_str::<discount_core::Action>(bad_action).is_err());
    let bad_rule = r#"{"type": "DateWindow", "starts_at": null, "ends_at": null, "extra": true}"#;
    assert!(serde_json::from_str::<discount_core::Rule>(bad_rule).is_err());
    let ok_rule = r#"{"type": "DateWindow"}"#;
    assert!(
        serde_json::from_str::<discount_core::Rule>(ok_rule).is_ok(),
        "missing optional keys read as null"
    );
    let bad_mode = r#"{"type": "FirstMatch", "extra": 1}"#;
    assert!(serde_json::from_str::<discount_core::StackingMode>(bad_mode).is_err());
}
