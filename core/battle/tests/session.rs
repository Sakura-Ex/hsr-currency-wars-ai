//! 分步调用、外部选招、暂停稳定性与两个独立结算触发的回归。
mod common;

use battle_core::{
    decision::{Decision, DecisionPoint, Operation},
    diagnostics::simulate,
    engine::{BattleSession, BattleStep},
    expression::Expression,
    model::*,
    modifiers::{Lifetime, ModifierDefinition, Property, PropertyChange, StackMode, TickUnit},
};
use common::*;

fn decision(session: &mut BattleSession<'_>) -> Decision {
    match session.advance().unwrap() {
        BattleStep::Decision(value) => value.clone(),
        BattleStep::Finished(_) => panic!("expected a decision"),
    }
}

fn finish(mut session: BattleSession<'_>) -> BattleResult {
    loop {
        let (id, choice) = match session.advance().unwrap() {
            BattleStep::Decision(d) => (d.id, d.legal.first().cloned()),
            BattleStep::Finished(_) => return session.into_result().unwrap(),
        };
        session.submit(id, choice).unwrap();
    }
}

fn turn(session: &mut BattleSession<'_>) -> Decision {
    loop {
        let d = decision(session);
        if d.point != DecisionPoint::Insert {
            return d;
        }
        session.submit(d.id, None).unwrap();
    }
}

fn hook(owner: &str, event: BattleEvent, effects: Vec<Effect>) -> Trigger {
    Trigger {
        owner: owner.into(),
        event,
        subject: Some("hero".into()),
        action: None,
        uses: 1,
        effects,
        expires_on: None,
        duration: 0,
    }
}

#[test]
fn paused_reads_invalid_choices_and_stale_ids_leave_the_decision_unchanged() {
    let mut hero = unit("hero", 100.0);
    let mut skill = action("paid", vec![damage(10.0, Target::AllEnemies)]);
    skill.kind = ActionKind::Skill;
    skill.sp_cost = 1;
    hero.actions.insert(0, skill);
    let mut s = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    s.ally_skill_points.initial = 3;
    s.max_actions = 1;
    let input = s;
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let insert = decision(&mut session);
    assert_eq!(insert.point, DecisionPoint::Insert);
    session.submit(insert.id, None).unwrap();
    let d = decision(&mut session);
    assert_eq!(d.point, DecisionPoint::NormalTurn);
    assert_eq!(d.observation.ally_skill_points, 3);
    assert_eq!(d.observation.enemy_skill_points, 0);
    close(d.observation.remaining_av, 900.0);
    let encoded = serde_json::to_value(&d).unwrap();
    for _ in 0..3 {
        assert_eq!(
            serde_json::to_value(decision(&mut session)).unwrap(),
            encoded
        );
    }
    assert_eq!(
        session
            .submit(insert.id, d.legal.first().cloned())
            .unwrap_err(),
        "stale decision id"
    );
    assert_eq!(
        session
            .submit(
                d.id,
                Some(Operation {
                    actor: 999,
                    action: 999
                })
            )
            .unwrap_err(),
        "policy returned an illegal operation"
    );
    assert_eq!(
        session.submit(d.id, None).unwrap_err(),
        "turn policy declined all legal actions"
    );
    assert_eq!(
        serde_json::to_value(decision(&mut session)).unwrap(),
        encoded
    );
    session.submit(d.id, d.legal.first().cloned()).unwrap();
    assert_eq!(
        session.submit(d.id, d.legal.first().cloned()).unwrap_err(),
        "battle is not awaiting a decision"
    );
    let result = finish(session);
    assert_eq!(result.actions, 1);
    assert_eq!(result.ally_skill_points, 2);
    close(state(&result, "enemy").hp, 990.0);
}

#[test]
fn step_by_step_replays_the_full_existing_scenarios_and_independent_sessions() {
    for raw in [
        include_str!("../examples/duel.json"),
        include_str!("../examples/team-waves.json"),
        include_str!("../examples/revival-budget.json"),
        include_str!("../examples/anaxa-s2-full-battle.json"),
    ] {
        let input = battle_core::diagnostics::ScenarioFile::from_slice(raw.as_bytes())
            .unwrap()
            .battle;
        let mut left = BattleSession::with_trace(&input, 7, true).unwrap();
        let mut right = BattleSession::with_trace(&input, 42, true).unwrap();
        for session in [&mut left, &mut right] {
            let d = decision(session);
            session.submit(d.id, d.legal.first().cloned()).unwrap();
        }
        for (seed, session) in [(42, right), (7, left)] {
            let result = finish(session);
            let expected = simulate(&input, seed, true).unwrap();
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }
}

#[test]
fn an_external_policy_can_choose_a_different_action_without_changing_the_engine() {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("basic", vec![damage(1.0, Target::AllEnemies)])];
    let mut skill = action("skill", vec![damage(1000.0, Target::AllEnemies)]);
    skill.kind = ActionKind::Skill;
    skill.sp_cost = 1;
    hero.actions.push(skill);
    let mut enemy = unit("enemy", 50.0);
    enemy.actions = vec![action("kill", vec![damage(1000.0, Target::AllEnemies)])];
    let mut s = scenario(vec![hero], vec![vec![enemy]]);
    s.ally_skill_points.initial = 3;
    let input = s;
    assert_eq!(simulate(&input, 7, false).unwrap().outcome, Outcome::Defeat);
    let mut session = BattleSession::with_trace(&input, 7, false).unwrap();
    let d = turn(&mut session);
    session.submit(d.id, d.legal.last().cloned()).unwrap();
    let result = finish(session);
    assert_eq!(result.outcome, Outcome::Victory);
    assert_eq!(result.actions, 1);
    assert_eq!(result.ally_skill_points, 2);
}

#[test]
fn selecting_an_ultimate_queues_it_and_payment_happens_on_actual_turn_entry() {
    let mut hero = unit("hero", 100.0);
    hero.initial_energy = 100.0;
    let mut ultimate = action("ultimate", vec![damage(1000.0, Target::AllEnemies)]);
    ultimate.kind = ActionKind::Ultimate;
    ultimate.energy_cost = 100.0;
    hero.actions.insert(0, ultimate);
    let input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = decision(&mut session);
    assert_eq!(d.point, DecisionPoint::Insert);
    close(d.observation.units[0].energy, 100.0);
    session.submit(d.id, d.legal.first().cloned()).unwrap();
    let result = finish(session);
    assert_eq!(result.outcome, Outcome::Victory);
    close(state(&result, "hero").energy, 0.0);
    assert_eq!(
        result
            .events
            .iter()
            .filter(|e| e.kind == "energy_cost")
            .count(),
        1
    );
    assert_eq!(result.actions, 1);
}

#[test]
fn cancellation_while_waiting_returns_an_incomplete_snapshot_without_an_action() {
    let input = scenario(vec![unit("hero", 100.0)], vec![vec![unit("enemy", 1.0)]]);
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = turn(&mut session);
    close(d.observation.now, 100.0);
    match session.advance_cancellable(&mut || true).unwrap() {
        BattleStep::Finished(result) => {
            assert_eq!(result.termination, Termination::Cancelled);
            assert_eq!(result.outcome, Outcome::Incomplete);
            assert_eq!(result.actions, 0);
            close(result.elapsed_av, 100.0);
        }
        BattleStep::Decision(_) => panic!("cancellation must end this session"),
    }
    assert_eq!(session.into_result().unwrap().actions, 0);
}

#[test]
fn cancellation_after_insert_submission_prevents_time_advance_and_ultimate_payment() {
    for choose_ultimate in [false, true] {
        let mut hero = unit("hero", 100.0);
        hero.initial_energy = 100.0;
        if choose_ultimate {
            let mut ultimate = action("ultimate", vec![damage(1000.0, Target::AllEnemies)]);
            ultimate.kind = ActionKind::Ultimate;
            ultimate.energy_cost = 100.0;
            hero.actions.insert(0, ultimate);
        }
        let input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
        let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
        let d = decision(&mut session);
        assert_eq!(d.point, DecisionPoint::Insert);
        let choice = if choose_ultimate {
            d.legal.first().cloned()
        } else {
            None
        };
        session.submit(d.id, choice).unwrap();
        let mut polls = 0;
        match session
            .advance_cancellable(&mut || {
                polls += 1;
                true
            })
            .unwrap()
        {
            BattleStep::Finished(result) => {
                assert_eq!(result.outcome, Outcome::Incomplete);
                assert_eq!(result.termination, Termination::Cancelled);
                assert_eq!(result.actions, 0);
                close(result.elapsed_av, 0.0);
                close(state(result, "hero").energy, 100.0);
                close(state(result, "enemy").hp, 1000.0);
                assert!(!result.events.iter().any(|e| e.kind == "energy_cost"));
            }
            BattleStep::Decision(_) => panic!("resume must observe cancellation first"),
        }
        assert_eq!(polls, 1);
        let mut polled_again = false;
        assert!(matches!(
            session
                .advance_cancellable(&mut || {
                    polled_again = true;
                    true
                })
                .unwrap(),
            BattleStep::Finished(_)
        ));
        assert!(!polled_again);
    }
}

#[test]
fn execution_error_ends_the_session_and_cannot_be_retried_on_partially_changed_state() {
    let mut hero = unit("hero", 100.0);
    hero.actions[0].effects = vec![Effect::DynamicEnergy {
        amount: Expression::Variable {
            key: "missing".into(),
        },
        target: Target::SelfUnit,
    }];
    let input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = turn(&mut session);
    let error = session.submit(d.id, d.legal.first().cloned()).unwrap_err();
    assert_eq!(error, "unresolved dynamic key missing");
    assert_eq!(session.advance().unwrap_err(), error);
    assert_eq!(session.into_result().unwrap_err(), error);
}

fn exhausted_during_action() -> Scenario {
    let mut hero = unit("hero", 100.0);
    hero.revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 50.0,
    });
    hero.actions = vec![action("strike", vec![damage(1.0, Target::AllEnemies)])];
    let mut s = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    s.max_av = 150.0;
    s.max_actions = 1;
    // 显式命中监听造成伤害，用于软件调度回归；没有隐含自伤或真实技能认定。
    s.mechanics.triggers = vec![hook(
        "enemy",
        BattleEvent::Hit,
        vec![damage(1000.0, Target::AllEnemies)],
    )];
    s
}

#[test]
fn zero_budget_does_not_interrupt_an_entered_turn_and_turn_end_precedes_cycle_end() {
    let mut s = exhausted_during_action();
    s.mechanics.triggers.push(hook(
        "hero",
        BattleEvent::TurnEnd,
        vec![Effect::Energy {
            amount: 7.0,
            target: Target::SelfUnit,
        }],
    ));
    let input = s;
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = turn(&mut session);
    assert_eq!(d.point, DecisionPoint::NormalTurn);
    close(d.observation.cycle_remaining_av, 50.0);
    close(d.observation.units[0].hp, 1000.0);
    session.submit(d.id, d.legal.first().cloned()).unwrap();
    let result = finish(session);
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    assert_eq!(result.outcome, Outcome::Defeat);
    assert_eq!(result.actions, 1);
    close(state(&result, "hero").energy, 7.0);
    close(result.elapsed_av, 100.0);
    close(result.penalty_av, 50.0);
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
    assert!(turn_end < cycle_end);
}

#[test]
fn elimination_is_an_independent_trigger_and_can_finish_before_the_countdown_turn() {
    for kill_at_end in [false, true] {
        let mut s = exhausted_during_action();
        if kill_at_end {
            s.mechanics.triggers.push(hook(
                "hero",
                BattleEvent::TurnEnd,
                vec![damage(1000.0, Target::AllEnemies)],
            ));
        } else {
            s.allies[0].actions[0].effects = vec![
                damage(1.0, Target::AllEnemies),
                damage(1000.0, Target::AllEnemies),
            ];
        }
        let result = simulate(&s, 7, true).unwrap();
        assert_eq!(result.outcome, Outcome::Victory);
        assert_eq!(result.termination, Termination::EnemiesDefeated);
        assert_eq!(result.actions, 1);
        close(result.remaining_av, 0.0);
        assert!(!result.events.iter().any(|e| e.kind == "cycle_end"));
    }
}

#[test]
fn zero_budget_preserves_insert_priority_and_turn_owned_modifiers_until_followup() {
    for unit_before_cycle in [false, true] {
        let mut hero = unit("hero", 100.0);
        let mut follow = action("follow", vec![damage(600.0, Target::AllEnemies)]);
        follow.kind = ActionKind::FollowUp;
        hero.actions = vec![
            action(
                "hit",
                vec![
                    damage(1000.0, Target::AllEnemies),
                    Effect::Request {
                        kind: RequestKind::FollowUp,
                        action: Some("follow".into()),
                        target: Target::SelfUnit,
                    },
                ],
            ),
            follow,
        ];
        let mut enemy = unit("enemy", 1.0);
        enemy.revival = Some(Revival {
            charges: 1,
            hp_fraction: 1.0,
            av_penalty: 50.0,
        });
        let mut s = scenario(vec![hero], vec![vec![enemy]]);
        s.max_av = 150.0;
        s.mechanics.unit_before_cycle = unit_before_cycle;
        s.mechanics.modifier_definitions.insert(
            "buff".into(),
            ModifierDefinition {
                stack_mode: StackMode::ReplaceBySource,
                max_stacks: 1,
                lifetime: Some(Lifetime {
                    event: BattleEvent::TurnEnd,
                    unit: TickUnit::Owner,
                    ticks: 1,
                }),
                properties: vec![PropertyChange {
                    property: Property::DamageBonus,
                    value: Expression::Constant { value: 1.0 },
                }],
            },
        );
        let mut start = hook(
            "hero",
            BattleEvent::BattleStart,
            vec![Effect::AddModifier {
                id: "buff".into(),
                stacks: Expression::Constant { value: 1.0 },
                target: Target::SelfUnit,
            }],
        );
        start.subject = None;
        s.mechanics.triggers.push(start);
        let result = simulate(&s, 7, true).unwrap();
        close(result.remaining_av, 0.0);
        if unit_before_cycle {
            assert_eq!(result.outcome, Outcome::Victory);
            assert_eq!(result.actions, 2);
            close(state(&result, "enemy").hp, 0.0);
            let hit = result
                .events
                .iter()
                .find(|e| e.kind == "damage" && e.action.as_deref() == Some("follow"))
                .unwrap();
            assert_eq!(hit.amount, Some(1200.0));
            assert!(!result.events.iter().any(|e| e.kind == "modifier_expired"
                || e.kind == "normal_turn_end"
                || e.kind == "cycle_end"));
        } else {
            assert_eq!(result.outcome, Outcome::Defeat);
            assert_eq!(result.termination, Termination::AvBudgetExhausted);
            assert_eq!(result.actions, 1);
            close(state(&result, "enemy").hp, 1000.0);
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
            assert!(turn_end < cycle_end);
        }
    }
}

#[test]
fn zero_time_turn_bookkeeping_finishes_before_new_action_limits_or_cancellation() {
    for cancel_after_action in [false, true] {
        let mut s = scenario(vec![unit("hero", 100.0)], vec![vec![unit("enemy", 1.0)]]);
        s.max_av = 100.0;
        s.max_actions = if cancel_after_action { 1000 } else { 1 };
        s.mechanics.unit_before_cycle = true;
        s.mechanics.triggers.push(hook(
            "hero",
            BattleEvent::TurnEnd,
            vec![Effect::Energy {
                amount: 7.0,
                target: Target::SelfUnit,
            }],
        ));
        let input = s;
        let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
        let d = turn(&mut session);
        close(d.observation.cycle_remaining_av, 0.0);
        session.submit(d.id, d.legal.first().cloned()).unwrap();
        let mut polls = 0;
        match session
            .advance_cancellable(&mut || {
                polls += 1;
                cancel_after_action
            })
            .unwrap()
        {
            BattleStep::Finished(result) => {
                assert_eq!(result.outcome, Outcome::Defeat);
                assert_eq!(result.termination, Termination::AvBudgetExhausted);
                assert_eq!(result.actions, 1);
                close(state(result, "hero").energy, 7.0);
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
                assert!(turn_end < cycle_end);
            }
            BattleStep::Decision(_) => panic!("only turn/cycle bookkeeping remains"),
        }
        assert_eq!(polls, 0);
    }
}

#[test]
fn declining_the_last_insert_finishes_the_expired_turn_before_new_cancellation() {
    let mut hero = unit("hero", 100.0);
    hero.actions[0].energy_gain = 100.0;
    let mut ultimate = action("ultimate", vec![damage(1.0, Target::AllEnemies)]);
    ultimate.kind = ActionKind::Ultimate;
    ultimate.energy_cost = 100.0;
    hero.actions.push(ultimate);
    let mut s = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
    s.max_av = 100.0;
    s.mechanics.unit_before_cycle = true;
    let input = s;
    let mut session = BattleSession::with_trace(&input, 7, true).unwrap();
    let d = turn(&mut session);
    session.submit(d.id, d.legal.first().cloned()).unwrap();
    let insert = decision(&mut session);
    assert_eq!(insert.point, DecisionPoint::Insert);
    assert_eq!(insert.legal.len(), 1);
    close(insert.observation.cycle_remaining_av, 0.0);
    session.submit(insert.id, None).unwrap();
    let mut polls = 0;
    match session
        .advance_cancellable(&mut || {
            polls += 1;
            true
        })
        .unwrap()
    {
        BattleStep::Finished(result) => {
            assert_eq!(result.termination, Termination::AvBudgetExhausted);
            assert_eq!(result.actions, 1);
            close(state(result, "hero").energy, 100.0);
            assert!(result.events.iter().any(|e| e.kind == "normal_turn_end"));
            assert!(result.events.iter().any(|e| e.kind == "cycle_end"));
        }
        BattleStep::Decision(_) => panic!("declined insert leaves only turn/cycle bookkeeping"),
    }
    assert_eq!(polls, 0);
}
