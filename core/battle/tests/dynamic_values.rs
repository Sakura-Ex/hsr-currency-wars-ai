mod common;
use battle_core::{diagnostics::simulate, expression::Expression, model::*};
use common::*;

fn load(key: &str) -> Expression {
    Expression::Variable { key: key.into() }
}
fn number(value: f64) -> Expression {
    Expression::Constant { value }
}

#[test]
fn dynamic_updates_are_ordered_runtime_state_and_visible_to_later_effects() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "count",
        vec![
            Effect::SetDynamicValue {
                key: "-169675107".into(),
                value: Expression::Add {
                    left: Box::new(load("-169675107")),
                    right: Box::new(number(1.0)),
                },
                target: Target::SelfUnit,
            },
            Effect::DynamicEnergy {
                amount: load("-169675107"),
                target: Target::SelfUnit,
            },
        ],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics
        .dynamic_values
        .insert("a".into(), [("-169675107".into(), 0.0)].into());
    s.max_actions = 3;
    let validated = s;
    for trace in [false, true] {
        let r = simulate(&validated, 7, trace).unwrap();
        close(state(&r, "a").energy, 6.0);
        close(state(&r, "a").dynamic_values["-169675107"], 3.0);
    }
}

#[test]
fn missing_runtime_values_are_errors_not_zero() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "read",
        vec![Effect::DynamicEnergy {
            amount: load("missing"),
            target: Target::SelfUnit,
        }],
    )];
    let checked = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    assert_eq!(
        simulate(&checked, 7, false).unwrap_err(),
        "unresolved dynamic key missing"
    );
}

#[test]
fn invalid_dynamic_arithmetic_is_not_committed() {
    let expression = Expression::Divide {
        left: Box::new(number(1.0)),
        right: Box::new(number(0.0)),
    };
    assert_eq!(
        expression.evaluate(&|_| None).unwrap_err(),
        "dynamic division by zero"
    );
    let overflow = Expression::Multiply {
        left: Box::new(number(f64::MAX)),
        right: Box::new(number(2.0)),
    };
    assert_eq!(
        overflow.evaluate(&|_| None).unwrap_err(),
        "nonfinite dynamic expression result"
    );
}

#[test]
fn pursuit_floor_and_weakness_weight_clamp_keep_boundary_behavior() {
    for (layer, expected) in [(0.0, 0.0), (4.0, 0.0), (5.0, 1.0), (9.0, 1.0), (10.0, 2.0)] {
        let expr = Expression::Floor {
            value: Box::new(Expression::Divide {
                left: Box::new(number(layer)),
                right: Box::new(number(5.0)),
            }),
        };
        close(expr.evaluate(&|_| None).unwrap(), expected);
    }
    for (weight, expected) in [(1000.0, 999.0), (0.0009999999, 0.0), (0.0, 0.0)] {
        let expr = Expression::Maximum {
            left: Box::new(Expression::Subtract {
                left: Box::new(number(weight)),
                right: Box::new(number(1.0)),
            }),
            right: Box::new(number(0.0)),
        };
        close(expr.evaluate(&|_| None).unwrap(), expected);
    }
}
