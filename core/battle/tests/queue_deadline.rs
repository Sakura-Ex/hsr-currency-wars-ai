//! AV归零时，完整Action后的队列补位先于TurnEnd与后续插入判断。
mod common;

use battle_core::{
    decision::DecisionPoint,
    diagnostics::simulate,
    engine::{BattleSession, BattleStep},
    model::*,
};
use common::*;

fn queued_at_deadline(before_cycle: bool, revival: bool, turn_end: Effect) -> Scenario {
    let mut hero = unit("hero", 100.0);
    let mut effects = vec![damage(1000.0, Target::AllEnemies)];
    let mut first = unit("e1", 1.0);
    if revival {
        first.revival = Some(Revival {
            charges: 1,
            hp_fraction: 1.0,
            av_penalty: 50.0,
        });
        effects.push(damage(1000.0, Target::AllEnemies));
    }
    hero.actions = vec![action("strike", effects)];
    let mut s = scenario(vec![hero], vec![vec![first, unit("e2", 1.0)]]);
    s.max_av = if revival { 150.0 } else { 100.0 };
    s.mechanics.queue_capacity = Some(1);
    s.mechanics.unit_before_cycle = before_cycle;
    s.mechanics.triggers.push(Trigger {
        owner: "hero".into(),
        event: BattleEvent::TurnEnd,
        subject: Some("hero".into()),
        action: None,
        uses: 1,
        effects: vec![turn_end],
        expires_on: None,
        duration: 0,
    });
    s
}

#[test]
fn queue_refills_after_the_complete_action_and_before_expired_turn_end() {
    for (before_cycle, revival) in [(true, false), (true, true), (false, true)] {
        let input = queued_at_deadline(before_cycle, revival, damage(1000.0, Target::AllEnemies));
        let result = simulate(&input, 7, true).unwrap();
        assert_eq!(result.outcome, Outcome::Victory);
        assert_eq!(result.termination, Termination::EnemiesDefeated);
        assert_eq!(result.actions, 1);
        assert_eq!(result.units.len(), 3);
        close(result.remaining_av, 0.0);
        close(state(&result, "e2").hp, 0.0);
        let action_end = result
            .events
            .iter()
            .position(|e| e.kind == "action_end" && e.action.as_deref() == Some("strike"))
            .unwrap();
        let spawn = result
            .events
            .iter()
            .position(|e| e.kind == "queue_spawn" && e.actor == "e2")
            .unwrap();
        let final_hit = result
            .events
            .iter()
            .position(|e| e.kind == "damage" && e.target.as_deref() == Some("e2"))
            .unwrap();
        let turn_end = result
            .events
            .iter()
            .position(|e| e.kind == "normal_turn_end")
            .unwrap();
        assert!(action_end < spawn && spawn < final_hit && final_hit < turn_end);
        assert!(!result.events.iter().any(|e| e.kind == "damage"
            && e.action.as_deref() == Some("strike")
            && e.target.as_deref() == Some("e2")));
        assert!(!result.events.iter().any(|e| e.kind == "cycle_end"));
    }
}

#[test]
fn queue_refill_is_preserved_in_timeout_snapshots_without_turn_end_damage() {
    let mut s = queued_at_deadline(
        true,
        false,
        Effect::Energy {
            amount: 7.0,
            target: Target::SelfUnit,
        },
    );
    s.max_actions = 1;
    let result = simulate(&s, 7, true).unwrap();
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    assert_eq!(result.actions, 1);
    assert_eq!(result.units.len(), 3);
    close(state(&result, "hero").energy, 7.0);
    close(state(&result, "e2").hp, 1000.0);
    let queue = result.queue.as_ref().unwrap();
    assert_eq!(queue.active_slots, [Some("e2".into())]);
    assert!(queue.remaining_ids.is_empty());
    let spawn = result
        .events
        .iter()
        .position(|e| e.kind == "queue_spawn")
        .unwrap();
    let turn_end = result
        .events
        .iter()
        .position(|e| e.kind == "normal_turn_end")
        .unwrap();
    let cycle_end = result
        .events
        .iter()
        .position(|e| e.kind == "cycle_end")
        .unwrap();
    assert!(spawn < turn_end && turn_end < cycle_end);
}

#[test]
fn a_refilled_units_ultimate_is_available_before_deciding_that_only_turn_end_remains() {
    let mut s = queued_at_deadline(
        true,
        false,
        Effect::Energy {
            amount: 7.0,
            target: Target::SelfUnit,
        },
    );
    let arriving = &mut s.waves[0][1];
    arriving.initial_energy = 100.0;
    let mut ultimate = action("arrival_ultimate", vec![damage(1000.0, Target::AllEnemies)]);
    ultimate.kind = ActionKind::Ultimate;
    ultimate.energy_cost = 100.0;
    arriving.actions.insert(0, ultimate);
    let input = s;
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = match session.advance().unwrap() {
        BattleStep::Decision(d) => d.clone(),
        BattleStep::Finished(_) => panic!("expected initial insert window"),
    };
    session.submit(d.id, None).unwrap();
    let d = match session.advance().unwrap() {
        BattleStep::Decision(d) => d.clone(),
        BattleStep::Finished(_) => panic!("expected hero turn"),
    };
    assert_eq!(d.point, DecisionPoint::NormalTurn);
    session.submit(d.id, d.legal.first().cloned()).unwrap();
    let d = match session.advance().unwrap() {
        BattleStep::Decision(d) => d.clone(),
        BattleStep::Finished(_) => panic!("refilled enemy must be considered before turn end"),
    };
    assert_eq!(d.point, DecisionPoint::Insert);
    assert_eq!(d.observation.units.len(), 3);
    close(d.observation.units[0].energy, 0.0);
    let op = d.legal.first().unwrap();
    assert_eq!(d.observation.units[op.actor].id, "e2");
    session.submit(d.id, Some(op.clone())).unwrap();
    let result = match session.advance().unwrap() {
        BattleStep::Finished(result) => result,
        BattleStep::Decision(_) => panic!("arrival ultimate eliminates the hero"),
    };
    assert_eq!(result.outcome, Outcome::Defeat);
    assert_eq!(result.termination, Termination::AlliesDefeated);
    assert_eq!(result.actions, 2);
    close(state(result, "e2").energy, 0.0);
}
