#![allow(dead_code)]
//! Artificial fixtures. Values are chosen for hand calculation, not game kits.
use battle_core::model::*;

/// 构造无需暴击、可手算的固定伤害效果。
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

/// 构造无费用和收益的普攻动作。
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

/// 构造不改变资源的等待动作。
pub fn idle() -> Action {
    action(
        "wait",
        vec![Effect::Energy {
            amount: 0.0,
            target: Target::SelfUnit,
        }],
    )
}

/// 构造具有指定速度的最小合成人工单位。
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

/// 模拟局面层已组装完成的双方配置和初始预算。
pub fn scenario(allies: Vec<Unit>, waves: Vec<Vec<Unit>>) -> Scenario {
    Scenario {
        mechanics: Mechanics::default(),
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

/// 检查浮点结果与独立手算期望的绝对误差。
pub fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

/// 按单位 ID 取得最终状态，样例保证该单位已经入场。
pub fn state<'a>(result: &'a BattleResult, id: &str) -> &'a UnitResult {
    result
        .units
        .iter()
        .find(|unit| unit.id == id)
        .expect("unit in result")
}
