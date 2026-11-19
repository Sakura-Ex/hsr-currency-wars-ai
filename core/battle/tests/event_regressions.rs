mod common;
use battle_core::{diagnostics::simulate, model::*};
use common::*;

fn energy(amount: f64, target: Target) -> Effect {
    Effect::Energy { amount, target }
}
fn hook(owner: &str, event: BattleEvent, effects: Vec<Effect>) -> Trigger {
    Trigger {
        owner: owner.into(),
        event,
        subject: None,
        action: None,
        uses: 0,
        effects,
        expires_on: None,
        duration: 0,
    }
}
fn expiring(mut t: Trigger, event: BattleEvent, duration: u32) -> Trigger {
    t.expires_on = Some(event);
    t.duration = duration;
    t
}
fn run(s: Scenario) -> BattleResult {
    simulate(&s, 7, true).unwrap()
}

#[test]
fn own_death_listener_runs_but_dead_owner_ignores_nested_other_death() {
    let mut e = unit("e", 200.0);
    e.actions = vec![action("kill", vec![damage(1000.0, Target::EnemyLowestHp)])];
    let mut s = scenario(vec![unit("a", 100.0), unit("b", 1.0)], vec![vec![e]]);
    s.mechanics.triggers = vec![
        hook(
            "a",
            BattleEvent::Death,
            vec![
                damage(1000.0, Target::AllEnemies),
                energy(7.0, Target::AllAllies),
            ],
        ),
        hook(
            "a",
            BattleEvent::ActionEnd,
            vec![energy(99.0, Target::AllAllies)],
        ),
    ];
    let r = run(s);
    assert_eq!(r.outcome, Outcome::Victory);
    close(state(&r, "a").hp, 0.0);
    close(state(&r, "e").hp, 0.0);
    close(state(&r, "b").energy, 7.0);
    assert_eq!(r.events.iter().filter(|e| e.kind == "death").count(), 2);
}

#[test]
fn global_lifetimes_expire_all_present_owners_before_any_nested_callback() {
    for global in [
        BattleEvent::BattleStart,
        BattleEvent::WaveStart,
        BattleEvent::CycleEnd,
    ] {
        let mut s = scenario(
            vec![unit("a", 100.0), unit("b", 100.0)],
            vec![vec![unit("e", 100.0)]],
        );
        s.max_av = 300.0;
        s.max_actions = 3;
        s.mechanics.cycles = vec![50.0, 250.0];
        let mut attack = hook("a", global, vec![damage(1.0, Target::AllEnemies)]);
        attack.uses = 1;
        s.mechanics.triggers.push(attack);
        for owner in ["a", "b", "e"] {
            let mut t = hook(
                owner,
                BattleEvent::ActionEnd,
                vec![energy(7.0, Target::SelfUnit)],
            );
            t.subject = Some(owner.into());
            s.mechanics.triggers.push(expiring(t, global, 1));
        }
        s.mechanics.triggers.push(expiring(
            hook("b", BattleEvent::Hit, vec![energy(11.0, Target::SelfUnit)]),
            global,
            1,
        ));
        let r = run(s);
        for owner in ["a", "b", "e"] {
            close(state(&r, owner).energy, 0.0);
        }
    }
}

#[test]
fn unit_lifetime_ticks_only_on_its_owner_event() {
    let mut s = scenario(
        vec![unit("a", 150.0), unit("b", 100.0)],
        vec![vec![unit("e", 1.0)]],
    );
    s.max_actions = 2;
    s.mechanics.triggers = vec![expiring(
        hook(
            "b",
            BattleEvent::ActionEnd,
            vec![energy(7.0, Target::SelfUnit)],
        ),
        BattleEvent::TurnStart,
        1,
    )];
    let r = run(s);
    close(state(&r, "b").energy, 7.0);
}

#[test]
fn future_wave_owner_does_not_expire_before_entry() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action("kill", vec![damage(1000.0, Target::AllEnemies)])];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0)], vec![unit("e2", 1.0)]]);
    s.max_av = 300.0;
    s.max_actions = 1;
    s.mechanics.cycles = vec![50.0, 250.0];
    s.mechanics.triggers = vec![expiring(
        hook(
            "e2",
            BattleEvent::WaveStart,
            vec![energy(7.0, Target::SelfUnit)],
        ),
        BattleEvent::CycleEnd,
        1,
    )];
    let r = run(s);
    close(state(&r, "e2").energy, 7.0);
}

#[test]
fn nested_hit_settles_each_death_once_and_keeps_remaining_action_effects() {
    for outer_hit in [1.0, 1000.0] {
        let mut a = unit("a", 100.0);
        a.actions = vec![action(
            "aoe",
            vec![
                damage(outer_hit, Target::AllEnemies),
                energy(5.0, Target::SelfUnit),
            ],
        )];
        let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
        let mut hit = hook(
            "a",
            BattleEvent::Hit,
            vec![damage(1000.0, Target::AllEnemies)],
        );
        hit.uses = 1;
        s.mechanics.triggers = vec![
            hit,
            hook("a", BattleEvent::Death, vec![energy(7.0, Target::SelfUnit)]),
        ];
        let r = run(s.clone());
        close(state(&r, "a").energy, 19.0);
        for victim in ["e1", "e2"] {
            close(state(&r, victim).hp, 0.0);
            assert_eq!(
                r.events
                    .iter()
                    .filter(|e| e.kind == "death" && e.actor == victim)
                    .count(),
                1
            );
        }
        let losses: Vec<_> = r
            .events
            .iter()
            .filter(|e| e.kind == "hp_loss")
            .map(|e| e.amount.unwrap())
            .collect();
        assert_eq!(
            losses,
            if outer_hit == 1.0 {
                vec![1.0, 999.0, 1000.0]
            } else {
                vec![1000.0, 1000.0]
            }
        );
        let quiet = simulate(&s, 7, false).unwrap();
        close(state(&quiet, "a").energy, 19.0);
    }
}

#[test]
fn nested_break_revalidates_remaining_targets_after_immediate_kills() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "break",
        vec![
            Effect::Toughness {
                amount: 30.0,
                target: Target::AllEnemies,
            },
            energy(5.0, Target::SelfUnit),
        ],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
    for victim in ["e1", "e2"] {
        s.mechanics.toughness.insert(victim.into(), 30.0);
    }
    s.mechanics.triggers = vec![
        hook(
            "a",
            BattleEvent::Break,
            vec![damage(1000.0, Target::AllEnemies)],
        ),
        hook("a", BattleEvent::Death, vec![energy(7.0, Target::SelfUnit)]),
    ];
    let r = run(s);
    assert_eq!(r.events.iter().filter(|e| e.kind == "break").count(), 1);
    close(state(&r, "e2").toughness, 30.0);
    close(state(&r, "a").energy, 19.0);
}

#[test]
fn revival_allows_a_new_death_transition_on_the_next_hit() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "two_hits",
        vec![
            damage(1000.0, Target::AllEnemies),
            damage(1000.0, Target::AllEnemies),
        ],
    )];
    let mut e = unit("e", 1.0);
    e.revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 10.0,
    });
    let mut s = scenario(vec![a], vec![vec![e]]);
    s.mechanics.triggers = vec![hook(
        "e",
        BattleEvent::Death,
        vec![energy(7.0, Target::AllEnemies)],
    )];
    let r = run(s);
    close(state(&r, "a").energy, 14.0);
    close(state(&r, "e").hp, 0.0);
    assert_eq!(state(&r, "e").revivals_left, 0);
    assert_eq!(r.events.iter().filter(|e| e.kind == "death").count(), 2);
    assert_eq!(r.events.iter().filter(|e| e.kind == "revive").count(), 1);
    close(r.penalty_av, 10.0);
}
