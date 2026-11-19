//! 有限队列的主流程与诊断结算边界。
mod common;
use battle_core::{
    diagnostics::{simulate, ScenarioFile},
    model::*,
};
use common::*;

/// 构造同时两人在场、总共四名敌人的最小队列。
fn queued() -> Scenario {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("strike", vec![damage(1000.0, Target::AllEnemies)])];
    let mut input = scenario(
        vec![hero],
        vec![(0..4).map(|i| unit(&format!("e{i}"), 10.0)).collect()],
    );
    input.max_av = 500.0;
    input.mechanics.queue_capacity = Some(2);
    input
}

#[test]
fn queue_refills_after_action_and_clears_all_enemies() {
    let result = simulate(&queued(), 7, true).unwrap();
    assert_eq!((result.outcome, result.actions), (Outcome::Victory, 2));
    close(result.elapsed_av, 200.0);
    assert!(result.queue.unwrap().remaining_ids.is_empty());
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| e.kind == "queue_spawn")
            .count(),
        2
    );
}

#[test]
fn waiting_queue_prevents_clear_at_the_deadline() {
    let mut input = queued();
    input.max_av = 150.0;
    let result = simulate(&input, 7, false).unwrap();
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    assert_eq!(
        result.units.iter().filter(|u| !u.ally && u.alive).count(),
        2
    );
}

#[test]
fn development_settlement_is_applied_outside_the_engine() {
    let file =
        ScenarioFile::from_slice(include_bytes!("../examples/anaxa-s2-full-battle.json")).unwrap();
    let result = simulate(&file.battle, 7, true).unwrap();
    assert!(result.actions > 0);
    let report = file.report(result, 7);
    let settlement = report.settlement.unwrap();
    assert!(settlement.progress >= 0.0 && settlement.progress <= 1.0);
    assert!(settlement.hp_after.is_some());
}
