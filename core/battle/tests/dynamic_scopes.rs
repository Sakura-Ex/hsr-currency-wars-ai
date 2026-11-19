mod common;
use battle_core::{
    diagnostics::simulate, dynamic_scope::DynamicScope, expression::Expression, model::*,
};
use common::*;

fn num(value: f64) -> Expression {
    Expression::Constant { value }
}
fn read(scope: DynamicScope) -> Expression {
    Expression::ScopedVariable {
        scope,
        key: "_count".into(),
    }
}
fn set(scope: DynamicScope, value: Expression) -> Effect {
    Effect::SetScopedDynamicValue {
        scope,
        key: "_count".into(),
        value,
    }
}
fn copy(key: &str, value: Expression) -> Effect {
    Effect::SetDynamicValue {
        key: key.into(),
        value,
        target: Target::SelfUnit,
    }
}
fn frame(scope: DynamicScope, effects: Vec<Effect>) -> Effect {
    Effect::WithDynamicScope { scope, effects }
}
fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}

#[test]
fn repeated_ability_and_nested_template_invocations_isolate_same_named_counters() {
    use DynamicScope::{Ability, TaskTemplate};
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "skill",
        vec![Effect::Repeat {
            count: num(2.0),
            effects: vec![frame(
                Ability,
                vec![
                    set(Ability, num(0.0)),
                    Effect::Repeat {
                        count: num(4.0),
                        effects: vec![set(Ability, add(read(Ability), num(1.0)))],
                    },
                    frame(
                        Ability,
                        vec![
                            set(Ability, num(55.0)),
                            copy("inner_ability", read(Ability)),
                        ],
                    ),
                    frame(
                        TaskTemplate,
                        vec![
                            set(TaskTemplate, num(99.0)),
                            copy("template", read(TaskTemplate)),
                            set(Ability, add(read(Ability), num(1.0))),
                        ],
                    ),
                    copy("outer_ability", read(Ability)),
                    Effect::DynamicEnergy {
                        amount: read(Ability),
                        target: Target::SelfUnit,
                    },
                ],
            )],
        }],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics
        .dynamic_values
        .insert("a".into(), [("_count".into(), 90.0)].into());
    s.max_actions = 1;
    let input = s;
    for trace in [false, true] {
        let r = simulate(&input, 0, trace).unwrap();
        let a = state(&r, "a");
        close(a.energy, 10.0);
        assert_eq!(a.dynamic_values.get("_count"), Some(&90.0));
        assert_eq!(a.dynamic_values.get("outer_ability"), Some(&5.0));
        assert_eq!(a.dynamic_values.get("inner_ability"), Some(&55.0));
        assert_eq!(a.dynamic_values.get("template"), Some(&99.0));
    }
}

#[test]
fn inner_missing_value_never_falls_through_to_outer_frame_or_persistent_storage() {
    use DynamicScope::Ability;
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "skill",
        vec![frame(
            Ability,
            vec![
                set(Ability, num(4.0)),
                frame(Ability, vec![copy("leaked", read(Ability))]),
            ],
        )],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics
        .dynamic_values
        .insert("a".into(), [("_count".into(), 99.0)].into());
    assert_eq!(
        simulate(&s, 0, false).unwrap_err(),
        "unresolved Ability dynamic key _count"
    );
}

#[test]
fn frame_is_unavailable_after_invocation_and_cannot_be_created_by_a_write() {
    use DynamicScope::Ability;
    for (effects, expected) in [
        (
            vec![
                frame(Ability, vec![set(Ability, num(4.0))]),
                copy("leaked", read(Ability)),
            ],
            "unresolved Ability dynamic key _count",
        ),
        (
            vec![set(Ability, num(4.0))],
            "no active Ability dynamic scope for owner",
        ),
    ] {
        let mut a = unit("a", 100.0);
        a.actions = vec![action("skill", effects)];
        let s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
        assert_eq!(simulate(&s, 0, false).unwrap_err(), expected);
    }
}

#[test]
fn another_owner_cannot_read_the_callers_active_ability_frame() {
    use DynamicScope::Ability;
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "skill",
        vec![frame(
            Ability,
            vec![set(Ability, num(4.0)), damage(1.0, Target::AbilityTarget)],
        )],
    )];
    let mut s = scenario(vec![a, unit("b", 1.0)], vec![vec![unit("e", 1.0)]]);
    s.mechanics.triggers.push(Trigger {
        owner: "b".into(),
        event: BattleEvent::Hit,
        subject: Some("a".into()),
        action: None,
        uses: 1,
        effects: vec![copy("stolen", read(Ability))],
        expires_on: None,
        duration: 0,
    });
    assert_eq!(
        simulate(&s, 0, false).unwrap_err(),
        "unresolved Ability dynamic key _count"
    );
}
