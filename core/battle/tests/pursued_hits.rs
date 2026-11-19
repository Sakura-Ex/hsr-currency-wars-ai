mod common;
use battle_core::{diagnostics::simulate, elements::*, expression::Expression, model::*};
use common::*;

fn num(value: f64) -> Expression {
    Expression::Constant { value }
}
fn elemental(element: DamageElement, target: Target) -> Effect {
    Effect::ElementalDamage {
        scale: Scale::Attack,
        ratio: num(0.0),
        flat: num(100.0),
        element,
        attack_type: AttackType::Pursued,
        can_crit: false,
        target,
    }
}
fn profile() -> ElementProfile {
    ElementProfile {
        damage_bonus: Element::ALL.into_iter().map(|e| (e, 0.0)).collect(),
        resistance: Element::ALL.into_iter().map(|e| (e, 0.0)).collect(),
    }
}
fn install_panels(s: &mut Scenario) {
    for u in s.allies.iter().chain(s.waves.iter().flatten()) {
        s.mechanics.element_profiles.insert(u.id.clone(), profile());
    }
}

fn mark(key: &str) -> Effect {
    Effect::SetDynamicValue {
        key: key.into(),
        value: num(1.0),
        target: Target::SelfUnit,
    }
}
fn when_pursued(effects: Vec<Effect>) -> Effect {
    Effect::Conditional {
        condition: Condition::All {
            conditions: vec![
                Condition::ActionKind {
                    kind: ActionKind::Skill,
                },
                Condition::HitAttackType {
                    attack_type: AttackType::Pursued,
                },
            ],
        },
        then_effects: effects,
        else_effects: vec![],
    }
}

#[test]
fn attack_finish_retargets_only_living_targets_already_hit_in_this_action() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "attack",
        vec![damage(10.0, Target::AbilityTarget), Effect::AttackFinish],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
    s.mechanics.triggers.push(Trigger {
        owner: "a".into(),
        event: BattleEvent::AfterAttack,
        subject: Some("a".into()),
        action: None,
        uses: 0,
        effects: vec![Effect::Repeat {
            count: num(2.0),
            effects: vec![Effect::RetargetAttackRandom {
                effects: vec![elemental(DamageElement::RandomAll, Target::ContextTarget)],
                empty_effects: vec![],
            }],
        }],
        expires_on: None,
        duration: 0,
    });
    install_panels(&mut s);
    s.max_actions = 1;
    let checked = s.clone();
    for trace in [false, true] {
        let r = simulate(&checked, 9, trace).unwrap();
        close(state(&r, "e1").hp, 790.0);
        close(state(&r, "e2").hp, 1000.0);
    }
    s.waves[0][0].initial_hp = 5.0;
    let r = simulate(&s, 9, true).unwrap();
    close(state(&r, "e2").hp, 1000.0);
    assert_eq!(r.events.iter().filter(|e| e.kind == "damage").count(), 1);
}

#[test]
fn explicit_element_panel_replaces_scalar_resistance_and_does_not_leak_to_nested_hits() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "wind",
        vec![elemental(
            DamageElement::Fixed {
                element: Element::Wind,
            },
            Target::AbilityTarget,
        )],
    )];
    a.actions[0].kind = ActionKind::Skill;
    a.actions[0]
        .effects
        .push(when_pursued(vec![mark("leaked_event")]));
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    install_panels(&mut s);
    s.mechanics
        .element_profiles
        .get_mut("a")
        .unwrap()
        .damage_bonus
        .insert(Element::Wind, 0.5);
    s.mechanics
        .element_profiles
        .get_mut("e")
        .unwrap()
        .resistance
        .insert(Element::Wind, 0.2);
    s.mechanics.triggers.push(Trigger {
        owner: "a".into(),
        event: BattleEvent::Hit,
        subject: Some("a".into()),
        action: None,
        uses: 1,
        effects: vec![when_pursued(vec![
            damage(100.0, Target::EnemyLowestHp),
            when_pursued(vec![mark("restored_pursued_in_skill")]),
        ])],
        expires_on: None,
        duration: 0,
    });
    s.max_actions = 1;
    let r = simulate(&s, 0, true).unwrap();
    let hits: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .map(|e| e.amount.unwrap())
        .collect();
    assert_eq!(hits, vec![120.0, 100.0]);
    let kinds: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .map(|e| e.attack_type)
        .collect();
    assert_eq!(
        kinds,
        vec![Some(AttackType::Pursued), Some(AttackType::Direct)]
    );
    assert_eq!(
        state(&r, "a")
            .dynamic_values
            .get("restored_pursued_in_skill"),
        Some(&1.0)
    );
    assert!(!state(&r, "a").dynamic_values.contains_key("leaked_event"));
}

#[test]
fn lethal_pursuit_restores_hit_kind_after_nested_death_listener_damage() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "skill",
        vec![elemental(
            DamageElement::Fixed {
                element: Element::Wind,
            },
            Target::AbilityTarget,
        )],
    )];
    a.actions[0].kind = ActionKind::Skill;
    a.actions[0]
        .effects
        .push(when_pursued(vec![mark("leaked_event")]));
    let mut first = unit("e1", 1.0);
    first.initial_hp = 50.0;
    let mut s = scenario(vec![a], vec![vec![first, unit("e2", 1.0)]]);
    install_panels(&mut s);
    s.mechanics
        .element_profiles
        .get_mut("a")
        .unwrap()
        .damage_bonus
        .insert(Element::Wind, 0.5);
    s.mechanics.triggers.push(Trigger {
        owner: "a".into(),
        event: BattleEvent::Death,
        subject: Some("e1".into()),
        action: None,
        uses: 1,
        effects: vec![when_pursued(vec![
            damage(100.0, Target::EnemyLowestHp),
            when_pursued(vec![mark("restored_death_context")]),
        ])],
        expires_on: None,
        duration: 0,
    });
    s.max_actions = 1;
    let r = simulate(&s, 0, true).unwrap();
    let hits: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .map(|e| (e.amount.unwrap(), e.attack_type))
        .collect();
    assert_eq!(
        hits,
        vec![
            (150.0, Some(AttackType::Pursued)),
            (100.0, Some(AttackType::Direct))
        ]
    );
    close(state(&r, "e2").hp, 900.0);
    assert_eq!(
        state(&r, "a").dynamic_values.get("restored_death_context"),
        Some(&1.0)
    );
    assert!(!state(&r, "a").dynamic_values.contains_key("leaked_event"));
}

#[test]
fn missing_retarget_context_fails_during_execution() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "bad_target",
        vec![damage(100.0, Target::ContextTarget)],
    )];
    let input = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    assert_eq!(
        simulate(&input, 0, false).unwrap_err(),
        "context target used outside a retarget scope"
    );
}
