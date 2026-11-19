mod common;
use battle_core::{
    diagnostics::simulate, elements::Element, expression::Expression, model::*, modifiers::*,
};
use common::*;

fn add(id: &str) -> Effect {
    Effect::AddModifier {
        id: id.into(),
        stacks: Expression::Constant { value: 1.0 },
        target: Target::EnemyLowestHp,
    }
}
fn remove(id: &str, all: bool) -> Effect {
    Effect::RemoveModifier {
        id: id.into(),
        from_any_source: all,
        target: Target::EnemyLowestHp,
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
fn install(s: &mut Scenario, id: &str, elements: Vec<Element>, lifetime: Option<Lifetime>) {
    for u in s.allies.iter().chain(s.waves.iter().flatten()) {
        s.mechanics
            .natural_weaknesses
            .entry(u.id.clone())
            .or_default();
    }
    s.mechanics.modifier_weaknesses.insert(id.into(), elements);
    s.mechanics.modifier_definitions.insert(
        id.into(),
        ModifierDefinition {
            stack_mode: StackMode::ReplaceBySource,
            max_stacks: 1,
            lifetime,
            properties: vec![],
        },
    );
}

#[test]
fn natural_and_attached_weaknesses_are_unioned_without_implicit_resistance_change() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "hit",
        vec![add("all"), damage(100.0, Target::AbilityTarget)],
    )];
    let mut e = unit("e", 1.0);
    e.stats.resistance = 0.2;
    let mut s = scenario(vec![a], vec![vec![e]]);
    install(&mut s, "all", Element::ALL.to_vec(), None);
    s.mechanics
        .natural_weaknesses
        .insert("e".into(), vec![Element::Wind, Element::Fire]);
    s.max_actions = 1;
    let input = s;
    for trace in [false, true] {
        let r = simulate(&input, 3, trace).unwrap();
        let e = state(&r, "e");
        let w = e.weaknesses.as_ref().unwrap();
        assert_eq!(w.natural.len(), 2);
        assert_eq!(w.attached.len(), 7);
        assert_eq!(w.all.len(), 7);
        close(e.hp, 920.0);
    }
}

#[test]
fn refresh_and_source_removal_preserve_other_sources_and_natural_weaknesses() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action("remove_own", vec![remove("wind", false)])];
    let mut s = scenario(vec![a, unit("b", 1.0)], vec![vec![unit("e", 1.0)]]);
    install(&mut s, "wind", vec![Element::Wind], None);
    s.mechanics
        .natural_weaknesses
        .insert("e".into(), vec![Element::Fire]);
    s.mechanics.triggers = vec![
        mount("a", vec![add("wind"), add("wind")]),
        mount("b", vec![add("wind")]),
    ];
    s.max_actions = 1;
    let r = simulate(&s.clone(), 0, true).unwrap();
    let e = state(&r, "e");
    assert_eq!(e.modifiers.len(), 1);
    assert_eq!(e.modifiers[0].source, "b");
    assert_eq!(e.weaknesses.as_ref().unwrap().all.len(), 2);
    s.allies[0].actions[0].effects.push(remove("wind", true));
    let r = simulate(&s, 0, true).unwrap();
    let w = state(&r, "e").weaknesses.as_ref().unwrap();
    assert!(w.attached.is_empty());
    assert_eq!(w.all, [Element::Fire].into());
    let counts: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "weakness_count")
        .map(|e| e.amount.unwrap())
        .collect();
    assert_eq!(counts, vec![2.0, 1.0]);
}

#[test]
fn owner_expiry_is_observed_before_same_event_listener_attaches_another_element() {
    let mut s = scenario(vec![unit("a", 1.0)], vec![vec![unit("e", 100.0)]]);
    install(
        &mut s,
        "wind",
        vec![Element::Wind],
        Some(Lifetime {
            event: BattleEvent::TurnEnd,
            unit: TickUnit::Owner,
            ticks: 1,
        }),
    );
    install(&mut s, "ice", vec![Element::Ice], None);
    s.mechanics
        .natural_weaknesses
        .insert("e".into(), vec![Element::Physical]);
    s.mechanics.triggers = vec![
        mount("a", vec![add("wind")]),
        Trigger {
            owner: "a".into(),
            event: BattleEvent::TurnEnd,
            subject: Some("e".into()),
            action: None,
            uses: 1,
            effects: vec![add("ice")],
            expires_on: None,
            duration: 0,
        },
    ];
    s.max_actions = 2;
    let r = simulate(&s, 0, true).unwrap();
    let w = state(&r, "e").weaknesses.as_ref().unwrap();
    assert_eq!(w.all, [Element::Physical, Element::Ice].into());
    let counts: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "weakness_count")
        .map(|e| e.amount.unwrap())
        .collect();
    assert_eq!(counts, vec![2.0, 1.0, 2.0]);
}

#[test]
fn unknown_natural_input_is_distinct_from_empty() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.mechanics.natural_weaknesses.insert("e".into(), vec![]);
    s.max_actions = 1;
    let r = simulate(&s, 0, false).unwrap();
    assert!(state(&r, "a").weaknesses.is_none());
    assert!(state(&r, "e").weaknesses.as_ref().unwrap().all.is_empty());
}
