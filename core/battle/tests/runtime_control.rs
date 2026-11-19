mod common;
use battle_core::{diagnostics::simulate, expression::Expression, model::*};
use common::*;

fn num(value: f64) -> Expression {
    Expression::Constant { value }
}
fn var(key: &str) -> Expression {
    Expression::Variable { key: key.into() }
}
fn set(key: &str, value: Expression) -> Effect {
    Effect::SetDynamicValue {
        key: key.into(),
        value,
        target: Target::SelfUnit,
    }
}
fn add(key: &str, n: f64) -> Expression {
    Expression::Add {
        left: Box::new(var(key)),
        right: Box::new(num(n)),
    }
}

#[test]
fn counter_threshold_branches_read_updated_state_and_keep_fractional_progress() {
    let mut a = unit("a", 100.0);
    a.actions = vec![Action {
        kind: ActionKind::Skill,
        ..action(
            "skill",
            vec![
                set("count", add("count", 1.0)),
                Effect::Conditional {
                    condition: Condition::Compare {
                        left: var("count"),
                        comparison: Comparison::GreaterEqual,
                        right: num(2.0),
                    },
                    then_effects: vec![
                        set("count", add("count", -2.0)),
                        set("persistent", add("persistent", 1.0)),
                    ],
                    else_effects: vec![],
                },
            ],
        )
    }];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics.dynamic_values.insert(
        "a".into(),
        [("count".into(), 0.0), ("persistent".into(), 7.0)].into(),
    );
    s.max_actions = 5;
    for trace in [false, true] {
        let r = simulate(&s.clone(), 8, trace).unwrap();
        close(state(&r, "a").dynamic_values["count"], 1.0);
        close(state(&r, "a").dynamic_values["persistent"], 9.0);
    }
}

#[test]
fn loop_count_is_captured_once_and_missing_branch_value_stays_unread() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "loop",
        vec![Effect::Conditional {
            condition: Condition::ActionKind {
                kind: ActionKind::Basic,
            },
            then_effects: vec![Effect::Repeat {
                count: var("count"),
                effects: vec![
                    set("count", add("count", -1.0)),
                    Effect::Energy {
                        amount: 2.0,
                        target: Target::SelfUnit,
                    },
                ],
            }],
            else_effects: vec![set("unused", var("missing"))],
        }],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics
        .dynamic_values
        .insert("a".into(), [("count".into(), 3.0)].into());
    s.max_actions = 1;
    let r = simulate(&s, 1, false).unwrap();
    close(state(&r, "a").energy, 6.0);
    close(state(&r, "a").dynamic_values["count"], 0.0);
}

#[test]
fn runtime_loop_counts_remain_bounded() {
    for count in [-1.0, 0.5, 10001.0] {
        let mut a = unit("a", 100.0);
        a.actions = vec![action(
            "loop",
            vec![Effect::Repeat {
                count: num(count),
                effects: vec![],
            }],
        )];
        let input = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
        assert!(simulate(&input, 0, false)
            .unwrap_err()
            .contains("repeat count"));
    }
}

#[test]
fn nested_damage_keeps_one_action_target() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "hits",
        vec![Effect::Repeat {
            count: num(2.0),
            effects: vec![Effect::Damage {
                scale: Scale::Attack,
                ratio: 0.0,
                flat: 1e6,
                hits: 1,
                can_crit: false,
                target: Target::AbilityTarget,
            }],
        }],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
    s.max_actions = 1;
    let r = simulate(&s, 1, true).unwrap();
    close(state(&r, "e1").hp, 0.0);
    close(state(&r, "e2").hp, 1000.0);
    assert_eq!(r.events.iter().filter(|e| e.kind == "damage").count(), 1);
}
