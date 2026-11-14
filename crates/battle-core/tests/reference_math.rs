//! Synthetic vectors evaluated by the pinned MIT HSR_Nous expression engine.
use hcwa_battle_core::math::{damage, heal, shield, DamageInput};
use serde_json::Value;
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
#[test]
fn upstream_expression_oracle_matches_all_eighteen_cases() {
    let source: Value = serde_json::from_str(include_str!("fixtures/math-oracle.json")).unwrap();
    assert_eq!(
        source["source_commit"],
        "f8935ddb407b297732fbd3c624d55897e3e648b9"
    );
    let mut count = 0;
    for row in source["damage"].as_array().unwrap() {
        let input: DamageInput = serde_json::from_value(row["input"].clone()).unwrap();
        close(
            damage(&input, row["critical"].as_bool().unwrap())
                .unwrap()
                .amount,
            row["expected"].as_f64().unwrap(),
        );
        count += 1;
    }
    for row in source["heal"].as_array().unwrap() {
        let v = &row["input"];
        close(
            heal(
                v[0].as_f64().unwrap(),
                v[1].as_f64().unwrap(),
                v[2].as_f64().unwrap(),
            )
            .unwrap(),
            row["expected"].as_f64().unwrap(),
        );
        count += 1;
    }
    for row in source["shield"].as_array().unwrap() {
        let v = &row["input"];
        close(
            shield(v[0].as_f64().unwrap(), v[1].as_f64().unwrap()).unwrap(),
            row["expected"].as_f64().unwrap(),
        );
        count += 1;
    }
    assert_eq!(count, 18);
}
