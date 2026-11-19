mod common;
use battle_core::{diagnostics::simulate, expression::Expression, model::*, modifiers::*};
use common::*;

fn num(value: f64) -> Expression {
    Expression::Constant { value }
}

/// 合法初始面板也可能被动态状态推到非法乘区，执行时应返回错误。
#[test]
fn negative_runtime_property_totals_cannot_create_shields_or_healing() {
    for (property, expected) in [
        (Property::DamageBonus, "combined damage bonus must be >= -1"),
        (
            Property::CritDamage,
            "combined critical damage must be >= 0",
        ),
    ] {
        let mut hero = unit("hero", 100.0);
        hero.stats.crit_rate = 1.0;
        hero.actions = vec![action(
            "strike",
            vec![Effect::Damage {
                scale: Scale::Attack,
                ratio: 0.0,
                flat: 10.0,
                hits: 1,
                can_crit: true,
                target: Target::AbilityTarget,
            }],
        )];
        let mut input = scenario(vec![hero], vec![vec![unit("enemy", 1.0)]]);
        input.mechanics.modifier_definitions.insert(
            "negative".into(),
            ModifierDefinition {
                stack_mode: StackMode::ReplaceBySource,
                max_stacks: 1,
                lifetime: None,
                properties: vec![PropertyChange {
                    property,
                    value: num(-3.0),
                }],
            },
        );
        input.mechanics.triggers.push(Trigger {
            owner: "hero".into(),
            event: BattleEvent::BattleStart,
            subject: None,
            action: None,
            uses: 1,
            effects: vec![add("negative", Target::SelfUnit)],
            expires_on: None,
            duration: 0,
        });
        assert_eq!(simulate(&input, 7, false).unwrap_err(), expected);
    }
}
fn add(id: &str, target: Target) -> Effect {
    Effect::AddModifier {
        id: id.into(),
        stacks: num(1.0),
        target,
    }
}
fn definition(mode: StackMode, lifetime: Option<Lifetime>, value: f64) -> ModifierDefinition {
    ModifierDefinition {
        stack_mode: mode,
        max_stacks: 2,
        lifetime,
        properties: vec![PropertyChange {
            property: Property::DamageBonus,
            value: num(value),
        }],
    }
}
fn mount(owner: &str, effects: Vec<Effect>) -> Trigger {
    Trigger {
        owner: owner.into(),
        event: BattleEvent::BattleStart,
        subject: None,
        action: None,
        uses: 1,
        effects,
        expires_on: None,
        duration: 0,
    }
}

#[test]
fn sourced_property_values_apply_once_and_owner_expiry_restores_base() {
    let mut a = unit("a", 100.0);
    a.stats.crit_rate = 1.0;
    a.actions = vec![action(
        "strike",
        vec![Effect::Damage {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 100.0,
            hits: 1,
            can_crit: true,
            target: Target::AbilityTarget,
        }],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    let lifetime = Some(Lifetime {
        event: BattleEvent::TurnEnd,
        unit: TickUnit::Owner,
        ticks: 1,
    });
    s.mechanics.modifier_definitions.insert(
        "buff".into(),
        ModifierDefinition {
            stack_mode: StackMode::ReplaceBySource,
            max_stacks: 1,
            lifetime,
            properties: vec![
                PropertyChange {
                    property: Property::CritDamage,
                    value: num(0.6),
                },
                PropertyChange {
                    property: Property::DamageBonus,
                    value: num(0.3),
                },
            ],
        },
    );
    s.mechanics
        .triggers
        .push(mount("a", vec![add("buff", Target::SelfUnit)]));
    s.max_actions = 2;
    let r = simulate(&s, 4, true).unwrap();
    let hits: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .map(|e| e.amount.unwrap())
        .collect();
    close(hits[0], 100.0 * 1.3 * 2.6);
    close(hits[1], 100.0 * 2.0);
    assert!(state(&r, "a").modifiers.is_empty());
}

#[test]
fn stacking_sources_are_separate_and_explicit_removal_is_source_scoped() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "remove",
        vec![Effect::RemoveModifier {
            id: "buff".into(),
            from_any_source: false,
            target: Target::AllAllies,
        }],
    )];
    let mut s = scenario(vec![a, unit("b", 1.0)], vec![vec![unit("e", 1.0)]]);
    s.mechanics
        .modifier_definitions
        .insert("buff".into(), definition(StackMode::AddBySource, None, 0.3));
    s.mechanics.triggers = vec![
        mount(
            "a",
            vec![
                add("buff", Target::AllAllies),
                add("buff", Target::AllAllies),
                add("buff", Target::AllAllies),
            ],
        ),
        mount("b", vec![add("buff", Target::AllAllies)]),
    ];
    s.max_actions = 1;
    let r = simulate(&s, 4, true).unwrap();
    for id in ["a", "b"] {
        let mods = &state(&r, id).modifiers;
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].source, "b");
        assert_eq!(mods[0].stacks, 1);
    }
    assert_eq!(
        r.events
            .iter()
            .filter(|e| e.kind == "modifier_applied" && e.amount == Some(2.0))
            .count(),
        4
    );
}

#[test]
fn source_clock_expiry_happens_before_turn_end_listener_damage() {
    let mut s = scenario(
        vec![unit("a", 100.0), unit("b", 1.0)],
        vec![vec![unit("e", 1.0)]],
    );
    s.mechanics.modifier_definitions.insert(
        "buff".into(),
        definition(
            StackMode::ReplaceBySource,
            Some(Lifetime {
                event: BattleEvent::TurnEnd,
                unit: TickUnit::Source,
                ticks: 1,
            }),
            1.0,
        ),
    );
    s.mechanics
        .triggers
        .push(mount("a", vec![add("buff", Target::AllAllies)]));
    s.mechanics.triggers.push(Trigger {
        owner: "b".into(),
        event: BattleEvent::TurnEnd,
        subject: Some("a".into()),
        action: None,
        uses: 1,
        effects: vec![damage(100.0, Target::EnemyLowestHp)],
        expires_on: None,
        duration: 0,
    });
    // The action-limit checkpoint is before TurnEnd; allow that turn to finish.
    s.max_actions = 2;
    let r = simulate(&s, 4, true).unwrap();
    close(
        r.events
            .iter()
            .find(|e| e.kind == "damage")
            .unwrap()
            .amount
            .unwrap(),
        100.0,
    );
    assert!(state(&r, "b").modifiers.is_empty());
}

#[test]
fn a_hit_listener_property_change_affects_the_next_hit_in_the_same_action() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "multi",
        vec![Effect::Damage {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 100.0,
            hits: 2,
            can_crit: false,
            target: Target::AbilityTarget,
        }],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.mechanics.modifier_definitions.insert(
        "buff".into(),
        definition(StackMode::ReplaceBySource, None, 0.3),
    );
    s.mechanics.triggers.push(Trigger {
        owner: "a".into(),
        event: BattleEvent::Hit,
        subject: Some("a".into()),
        action: None,
        uses: 1,
        effects: vec![add("buff", Target::SelfUnit)],
        expires_on: None,
        duration: 0,
    });
    s.max_actions = 1;
    let r = simulate(&s, 4, true).unwrap();
    let hits: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .map(|e| e.amount.unwrap())
        .collect();
    assert_eq!(hits, vec![100.0, 130.0]);
}
