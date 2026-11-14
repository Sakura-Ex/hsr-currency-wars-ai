//! Small hand-checkable fixtures, independent of private game-data tables.
use hcwa_battle_core::math::{damage, heal, shield, DamageInput};
use serde_json::json;

fn baseline() -> DamageInput {
    DamageInput {
        base_damage: 1000.0,
        attacker_level: 80,
        defense: 1000.0,
        defense_reduction: 0.0,
        defense_ignore: 0.0,
        resistance: 0.0,
        resistance_penetration: 0.0,
        damage_bonus: 0.0,
        vulnerability: 0.0,
        toughness_broken: false,
        damage_reduction: 0.0,
        independent_multiplier: 1.0,
        crit_damage: 0.5,
    }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn hand_calculated_damage_and_injected_critical_outcome() {
    // Level term=1000, defense factor=1/2, unbroken factor=9/10.
    let input = baseline();
    let ordinary = damage(&input, false).unwrap();
    let critical = damage(&input, true).unwrap();
    close(ordinary.amount, 450.0);
    close(critical.amount, 675.0);
    close(ordinary.defense_multiplier, 0.5);
    close(ordinary.toughness_multiplier, 0.9);
    assert!(!ordinary.critical);
    assert!(critical.critical);
    assert_eq!(ordinary, damage(&input, false).unwrap());
}

#[test]
fn broken_flag_changes_only_its_multiplier() {
    let mut input = baseline();
    input.toughness_broken = true;
    let out = damage(&input, false).unwrap();
    close(out.amount, 500.0);
    close(out.toughness_multiplier, 1.0);
}

#[test]
fn defense_reduction_and_ignore_share_the_clamped_defense_pool() {
    let mut input = baseline();
    input.defense_reduction = 0.6;
    input.defense_ignore = 0.6;
    let out = damage(&input, false).unwrap();
    close(out.defense_multiplier, 1.0);
    close(out.amount, 900.0);
    input.defense_reduction = 0.2;
    input.defense_ignore = 0.3;
    close(damage(&input, false).unwrap().amount, 600.0);
    input.defense_reduction = 2.0;
    input.defense_ignore = 0.0;
    close(damage(&input, false).unwrap().amount, 900.0);
}

#[test]
fn resistance_clamps_the_effective_difference_before_multiplier() {
    let mut input = baseline();
    input.resistance = 2.0;
    close(damage(&input, false).unwrap().amount, 45.0);
    input.resistance = -2.0;
    close(damage(&input, false).unwrap().amount, 900.0);
    input.resistance = 0.2;
    input.resistance_penetration = 0.3;
    close(damage(&input, false).unwrap().amount, 495.0);
    input.resistance_penetration = -0.1;
    close(damage(&input, false).unwrap().amount, 315.0);
}

#[test]
fn bonus_vulnerability_and_reduction_are_separate_factors() {
    let mut input = baseline();
    input.damage_bonus = 1.0;
    input.vulnerability = 0.5;
    input.damage_reduction = 0.5;
    // 450 * 2 * 1.5 * 0.5, not 450*(1+1+0.5-0.5).
    close(damage(&input, false).unwrap().amount, 675.0);
    input.independent_multiplier = 2.0;
    close(damage(&input, false).unwrap().amount, 1350.0);
}

#[test]
fn explicit_zero_and_complete_reduction_are_valid() {
    let mut input = baseline();
    input.base_damage = 0.0;
    assert_eq!(damage(&input, true).unwrap().amount, 0.0);
    input = baseline();
    input.damage_reduction = 1.0;
    assert_eq!(damage(&input, true).unwrap().amount, 0.0);
    input = baseline();
    input.independent_multiplier = 0.0;
    assert_eq!(damage(&input, false).unwrap().amount, 0.0);
    input = baseline();
    input.damage_bonus = -1.0;
    assert_eq!(damage(&input, false).unwrap().amount, 0.0);
}

#[test]
fn heal_uses_additive_outgoing_and_incoming_and_returns_raw_amount() {
    // Preassembled base: 10% of 1000 ATK + 5% of 2000 HP + flat 20 = 220.
    close(heal(220.0, 0.1, 0.1).unwrap(), 264.0);
    close(heal(220.0, 0.0, -0.5).unwrap(), 110.0);
    assert_eq!(heal(100.0, -0.5, -0.5).unwrap(), 0.0);
    assert_eq!(heal(0.0, 2.0, 3.0).unwrap(), 0.0);
}

#[test]
fn shield_uses_preassembled_base_and_does_not_apply_stacking_or_caps() {
    // Preassembled base: 20% of 500 DEF + 5% of 2000 HP + flat 50 = 250.
    close(shield(250.0, 0.2).unwrap(), 300.0);
    close(shield(12.5, 0.3).unwrap(), 16.25);
    assert_eq!(shield(100.0, -1.0).unwrap(), 0.0);
}

fn set_numeric(input: &mut DamageInput, field: &str, value: f64) {
    match field {
        "base_damage" => input.base_damage = value,
        "defense" => input.defense = value,
        "defense_reduction" => input.defense_reduction = value,
        "defense_ignore" => input.defense_ignore = value,
        "resistance" => input.resistance = value,
        "resistance_penetration" => input.resistance_penetration = value,
        "damage_bonus" => input.damage_bonus = value,
        "vulnerability" => input.vulnerability = value,
        "damage_reduction" => input.damage_reduction = value,
        "independent_multiplier" => input.independent_multiplier = value,
        "crit_damage" => input.crit_damage = value,
        _ => panic!("unknown fixture field"),
    }
}

#[test]
fn every_numeric_input_rejects_nonfinite_values() {
    for field in [
        "base_damage",
        "defense",
        "defense_reduction",
        "defense_ignore",
        "resistance",
        "resistance_penetration",
        "damage_bonus",
        "vulnerability",
        "damage_reduction",
        "independent_multiplier",
        "crit_damage",
    ] {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut input = baseline();
            set_numeric(&mut input, field, value);
            assert!(damage(&input, false).is_err(), "{field}={value}");
        }
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(heal(value, 0.0, 0.0).is_err());
        assert!(heal(1.0, value, 0.0).is_err());
        assert!(heal(1.0, 0.0, value).is_err());
        assert!(shield(value, 0.0).is_err());
        assert!(shield(1.0, value).is_err());
    }
}

#[test]
fn invalid_ranges_fail_without_silent_repair() {
    for (field, value) in [
        ("base_damage", -1.0),
        ("defense", -1.0),
        ("defense_reduction", -0.1),
        ("defense_ignore", -0.1),
        ("damage_bonus", -1.1),
        ("vulnerability", -1.1),
        ("damage_reduction", 1.1),
        ("independent_multiplier", -0.1),
        ("crit_damage", -0.1),
    ] {
        let mut input = baseline();
        set_numeric(&mut input, field, value);
        assert!(damage(&input, false).is_err(), "{field}={value}");
    }
    let mut input = baseline();
    input.attacker_level = 0;
    assert!(damage(&input, false).is_err());
    assert!(heal(100.0, -0.75, -0.5).is_err());
    assert!(heal(-1.0, 0.0, 0.0).is_err());
    assert!(shield(100.0, -1.1).is_err());
}

#[test]
fn overflowing_intermediates_never_become_serialized_results() {
    let mut input = baseline();
    input.base_damage = f64::MAX;
    input.independent_multiplier = 2.0;
    assert!(damage(&input, false).is_err());
    input = baseline();
    input.resistance = -f64::MAX;
    input.resistance_penetration = f64::MAX;
    assert!(damage(&input, false).is_err());
    input = baseline();
    input.defense_reduction = f64::MAX;
    input.defense_ignore = f64::MAX;
    assert!(damage(&input, false).is_err());
    assert!(heal(f64::MAX, 1.0, 0.0).is_err());
    assert!(shield(f64::MAX, 1.0).is_err());
    serde_json::to_string(&damage(&baseline(), false).unwrap()).unwrap();
}

#[test]
fn json_contract_is_strict_and_keeps_unsupported_zones_out() {
    let value = serde_json::to_value(baseline()).unwrap();
    let parsed: DamageInput = serde_json::from_value(value.clone()).unwrap();
    close(damage(&parsed, false).unwrap().amount, 450.0);
    let mut extra = value.clone();
    extra["final_dmg_bonus"] = json!(1.0);
    assert!(serde_json::from_value::<DamageInput>(extra).is_err());
    for bad in [json!("1000"), Value::Null, json!(true)] {
        let mut invalid = value.clone();
        invalid["base_damage"] = bad;
        assert!(serde_json::from_value::<DamageInput>(invalid).is_err());
    }
}

use serde_json::Value;
