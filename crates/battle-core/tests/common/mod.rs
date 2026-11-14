#![allow(dead_code)]
//! Artificial fixtures. Values are chosen for hand calculation, not game kits.
use hcwa_battle_core::model::*;

pub fn damage(amount: f64, target: Target) -> Effect {
    Effect::Damage {
        scale: Scale::Attack,
        ratio: 0.0,
        flat: amount,
        hits: 1,
        can_crit: false,
        target,
    }
}

pub fn action(id: &str, effects: Vec<Effect>) -> Action {
    Action {
        id: id.into(),
        kind: ActionKind::Basic,
        sp_cost: 0,
        sp_gain: 0,
        energy_cost: 0.0,
        energy_gain: 0.0,
        effects,
    }
}

pub fn idle() -> Action {
    action(
        "wait",
        vec![Effect::Energy {
            amount: 0.0,
            target: Target::SelfUnit,
        }],
    )
}

pub fn unit(id: &str, speed: f64) -> Unit {
    Unit {
        id: id.into(),
        stats: Stats {
            level: 1,
            max_hp: 1000.0,
            attack: 10.0,
            defense: 0.0,
            speed,
            taunt: 1.0,
            crit_rate: 0.0,
            crit_damage: 1.0,
            damage_bonus: 0.0,
            defense_reduction: 0.0,
            defense_ignore: 0.0,
            resistance: 0.0,
            resistance_penetration: 0.0,
            vulnerability: 0.0,
            damage_reduction: 0.0,
            independent_multiplier: 1.0,
            toughness_broken: true,
            healing_bonus: 0.0,
            incoming_healing_bonus: 0.0,
            shield_bonus: 0.0,
        },
        initial_hp: 1000.0,
        initial_energy: 0.0,
        max_energy: 100.0,
        revival: None,
        actions: vec![idle()],
    }
}

pub fn scenario(allies: Vec<Unit>, waves: Vec<Vec<Unit>>) -> Scenario {
    Scenario {
        schema_version: 1,
        ruleset: "basic_v1".into(),
        provenance: Provenance {
            game_version: "synthetic".into(),
            data_snapshot: "hand-computed-tests-v1".into(),
            description: "Artificial panels and supported primitives; no game data".into(),
        },
        unsupported_mechanics: vec![],
        max_av: 1000.0,
        max_actions: 1000,
        ally_skill_points: SkillPoints {
            initial: 0,
            maximum: 5,
        },
        enemy_skill_points: SkillPoints {
            initial: 0,
            maximum: 5,
        },
        allies,
        waves,
    }
}

pub fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

pub fn state<'a>(result: &'a BattleResult, id: &str) -> &'a UnitResult {
    result
        .units
        .iter()
        .find(|unit| unit.id == id)
        .expect("unit in result")
}
