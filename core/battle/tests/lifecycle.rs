mod common;
use battle_core::{
    decision::*,
    diagnostics::{simulate, simulate_with_policy, ScenarioFile},
    engine::simulate as simulate_in_memory,
    model::*,
};
use common::*;

fn hook(owner: &str, event: BattleEvent, effects: Vec<Effect>) -> Trigger {
    Trigger {
        owner: owner.into(),
        event,
        subject: None,
        action: None,
        uses: 1,
        effects,
        expires_on: None,
        duration: 0,
    }
}
fn run(s: Scenario) -> BattleResult {
    simulate(&s, 7, true).unwrap()
}
fn request(kind: RequestKind, name: Option<&str>) -> Effect {
    Effect::Request {
        kind,
        action: name.map(str::to_owned),
        target: Target::SelfUnit,
    }
}

#[test]
fn initialization_and_each_wave_dispatch_real_effects() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action("kill", vec![damage(1000.0, Target::AllEnemies)])];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0)], vec![unit("e2", 1.0)]]);
    let mut wave = hook(
        "a",
        BattleEvent::WaveStart,
        vec![Effect::Energy {
            amount: 10.0,
            target: Target::SelfUnit,
        }],
    );
    wave.uses = 0;
    s.mechanics.triggers = vec![
        hook(
            "a",
            BattleEvent::BattleStart,
            vec![Effect::Energy {
                amount: 5.0,
                target: Target::SelfUnit,
            }],
        ),
        wave,
    ];
    let r = run(s);
    close(state(&r, "a").energy, 25.0);
    assert_eq!(r.waves_cleared, 2);
}
#[test]
fn ultimate_payment_precedes_turn_start_and_is_not_repeated() {
    for gain in [20.0, -100.0] {
        let mut a = unit("a", 100.0);
        a.initial_energy = 100.0;
        let mut ult = action("ult", vec![damage(1.0, Target::AllEnemies)]);
        ult.kind = ActionKind::Ultimate;
        ult.energy_cost = 100.0;
        a.actions.insert(0, ult);
        let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
        s.max_actions = 1;
        s.mechanics.triggers = vec![hook(
            "a",
            BattleEvent::TurnStart,
            vec![Effect::Energy {
                amount: gain,
                target: Target::SelfUnit,
            }],
        )];
        let r = run(s);
        close(state(&r, "a").energy, gain.max(0.0));
        close(r.elapsed_av, 0.0);
        let entry = r
            .events
            .iter()
            .find(|e| e.kind == "ultimate_turn_start")
            .unwrap();
        close(entry.amount.unwrap(), 0.0);
        assert_eq!(
            r.events.iter().filter(|e| e.kind == "energy_cost").count(),
            1
        );
        close(state(&r, "e").hp, 999.0);
    }
}
#[test]
fn complete_action_then_followup_without_turn_events_or_av() {
    let mut a = unit("a", 100.0);
    let mut fua = action("fua", vec![damage(7.0, Target::AllEnemies)]);
    fua.kind = ActionKind::FollowUp;
    a.actions = vec![
        action(
            "two_hits",
            vec![
                damage(3.0, Target::AllEnemies),
                damage(5.0, Target::AllEnemies),
            ],
        ),
        fua,
    ];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 2;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::Hit,
        vec![request(RequestKind::FollowUp, Some("fua"))],
    )];
    let r = run(s.clone());
    close(state(&r, "e").hp, 985.0);
    close(r.elapsed_av, 100.0);
    let action_end = r
        .events
        .iter()
        .position(|e| e.kind == "action_end" && e.action.as_deref() == Some("two_hits"))
        .unwrap();
    let follow = r
        .events
        .iter()
        .position(|e| e.kind == "action" && e.action.as_deref() == Some("fua"))
        .unwrap();
    assert!(action_end < follow);
    assert_eq!(
        r.events
            .iter()
            .filter(|e| e.kind.ends_with("turn_start"))
            .count(),
        1
    );
    let quiet = simulate(&s, 7, false).unwrap();
    close(state(&quiet, "e").hp, 985.0);
}
#[test]
fn extra_turn_selects_after_entry_and_keeps_normal_actor_at_axis_top() {
    let mut a = unit("a", 100.0);
    a.actions = vec![
        idle(),
        action("chosen_after_entry", vec![damage(1.0, Target::AllEnemies)]),
    ];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 2;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::ActionEnd,
        vec![request(RequestKind::ExtraTurn, None)],
    )];
    let mut extra_seen = false;
    let mut policy = |p: DecisionPoint, o: &Observation, legal: &[Operation]| {
        if p == DecisionPoint::ExtraTurn {
            extra_seen = true;
            close(
                o.axis
                    .iter()
                    .find(|x| x.id == "a")
                    .unwrap()
                    .remaining_distance,
                0.0,
            );
            legal.last().cloned()
        } else {
            legal.first().cloned()
        }
    };
    let r = simulate_with_policy(&s, 7, true, &mut || false, &mut policy).unwrap();
    assert!(extra_seen);
    close(r.elapsed_av, 100.0);
    close(state(&r, "e").hp, 999.0);
}
#[test]
fn mid_wait_speed_change_preserves_remaining_distance() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 1;
    s.max_av = 200.0;
    s.mechanics.cycles = vec![50.0, 150.0];
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::CycleEnd,
        vec![Effect::Speed {
            value: 200.0,
            target: Target::SelfUnit,
        }],
    )];
    let r = run(s);
    close(r.elapsed_av, 75.0); // 50 AV at 100 speed + 5000 / 200.
}
#[test]
fn turn_end_applies_explicit_next_action_shift_after_all_inserts() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 2;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::TurnEnd,
        vec![Effect::AdvanceNext {
            fraction: 0.25,
            target: Target::SelfUnit,
        }],
    )];
    let r = run(s);
    close(r.elapsed_av, 175.0);
}
#[test]
fn cycle_penalty_1t30_minus_50_caps_at_zero_without_distance_or_time_change() {
    let mut a = unit("a", 100.0);
    a.revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 50.0,
    });
    let mut e = unit("e", 1.0);
    let mut ult = action("kill", vec![damage(1000.0, Target::AllEnemies)]);
    ult.kind = ActionKind::Ultimate;
    ult.energy_cost = 100.0;
    e.initial_energy = 100.0;
    e.actions.insert(0, ult);
    let mut s = scenario(vec![a], vec![vec![e]]);
    s.max_av = 130.0;
    s.mechanics.cycles = vec![30.0, 100.0];
    s.max_actions = 1;
    let r = run(s);
    close(r.elapsed_av, 0.0);
    close(r.penalty_av, 30.0);
    assert_eq!(r.cycle, 2);
    close(r.cycle_remaining_av, 100.0);
    close(state(&r, "a").remaining_distance, 10000.0);
    close(state(&r, "e").remaining_distance, 10000.0);
}
#[test]
fn explicit_cycle_tie_policy_changes_only_the_documented_boundary() {
    for before in [false, true] {
        let mut a = unit("a", 100.0);
        a.actions = vec![action("kill", vec![damage(1000.0, Target::AllEnemies)])];
        let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
        s.max_av = 100.0;
        s.mechanics.unit_before_cycle = before;
        let r = run(s);
        assert_eq!(
            r.outcome,
            if before {
                Outcome::Victory
            } else {
                Outcome::Defeat
            }
        );
    }
}
#[test]
fn break_and_death_events_register_actions_at_action_boundary() {
    let mut a = unit("a", 100.0);
    let mut fua = action("fua", vec![damage(1.0, Target::AllEnemies)]);
    fua.kind = ActionKind::FollowUp;
    a.actions = vec![
        action(
            "break",
            vec![
                Effect::Toughness {
                    amount: 30.0,
                    target: Target::AllEnemies,
                },
                damage(5.0, Target::AllEnemies),
            ],
        ),
        fua,
    ];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 2;
    s.mechanics.toughness.insert("e".into(), 30.0);
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::Break,
        vec![request(RequestKind::FollowUp, Some("fua"))],
    )];
    let r = run(s);
    close(state(&r, "e").toughness, 0.0);
    close(state(&r, "e").hp, 994.0);
    assert_eq!(r.actions, 2);
}
#[test]
fn single_friendly_damage_factor_does_not_change_enemy_damage() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action("a", vec![damage(100.0, Target::AllEnemies)])];
    let mut e = unit("e", 100.0);
    e.actions = vec![action("e", vec![damage(100.0, Target::AllEnemies)])];
    let mut s = scenario(vec![a], vec![vec![e]]);
    s.max_actions = 2;
    for (factor, enemy_hp) in [(1.0, 900.0), (0.3, 970.0), (0.0, 1_000.0)] {
        s.mechanics.ally_damage_factor = factor;
        let memory = simulate_in_memory(&s, 7).unwrap();
        close(state(&memory, "e").hp, enemy_hp);
        close(state(&memory, "a").hp, 900.0);
        let mut json = serde_json::to_value(&s).unwrap();
        if factor == 1.0 {
            json["mechanics"]
                .as_object_mut()
                .unwrap()
                .remove("ally_damage_factor");
        }
        let file = ScenarioFile::from_slice(&serde_json::to_vec(&json).unwrap()).unwrap();
        let diagnostic = simulate(&file.battle, 7, false).unwrap();
        assert_eq!(
            serde_json::to_value(memory).unwrap(),
            serde_json::to_value(diagnostic).unwrap()
        );
    }
}

#[test]
fn invalid_friendly_damage_factor_cannot_change_enemy_shield() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action("a", vec![damage(100.0, Target::AllEnemies)])];
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 1;
    for factor in [-1.0, f64::NAN, f64::INFINITY] {
        s.mechanics.ally_damage_factor = factor;
        assert_eq!(
            simulate_in_memory(&s, 7).expect_err("invalid factor must prevent damage settlement"),
            "ally_damage_factor must be finite and non-negative"
        );
    }
    s.mechanics.ally_damage_factor = -1.0;
    let file = ScenarioFile::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert_eq!(
        simulate(&file.battle, 7, false).unwrap_err(),
        "ally_damage_factor must be finite and non-negative"
    );
}

#[test]
fn friendly_damage_without_a_live_target_does_not_apply_the_factor() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.mechanics.ally_damage_factor = -1.0;
    let mut after_death = hook(
        "a",
        BattleEvent::Death,
        vec![damage(100.0, Target::AllEnemies)],
    );
    after_death.subject = Some("e".into());
    s.mechanics.triggers = vec![
        hook(
            "e",
            BattleEvent::BattleStart,
            vec![damage(1000.0, Target::SelfUnit)],
        ),
        after_death,
    ];
    let result = run(s);
    assert_eq!(result.termination, Termination::EnemiesDefeated);
    assert_eq!(result.actions, 0);
    let damage_events: Vec<_> = result
        .events
        .iter()
        .filter(|e| e.kind == "damage")
        .collect();
    assert_eq!(damage_events.len(), 1);
    assert_eq!(damage_events[0].actor, "e");
    assert!(result
        .events
        .iter()
        .any(|e| e.kind == "trigger" && e.actor == "a"));
}
#[test]
fn event_status_expiry_and_illegal_policy_are_observable() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 2;
    let mut t = hook(
        "a",
        BattleEvent::ActionEnd,
        vec![Effect::Energy {
            amount: 7.0,
            target: Target::SelfUnit,
        }],
    );
    t.uses = 0;
    t.expires_on = Some(BattleEvent::TurnStart);
    t.duration = 2;
    s.mechanics.triggers = vec![t];
    let r = run(s.clone());
    close(state(&r, "a").energy, 7.0);
    let mut bad = |_: DecisionPoint, _: &Observation, _: &[Operation]| {
        Some(Operation {
            actor: 999,
            action: 999,
        })
    };
    assert!(simulate_with_policy(&s, 1, false, &mut || false, &mut bad)
        .unwrap_err()
        .contains("illegal operation"));
}

#[test]
fn config_bound_target_is_not_reselected_after_first_hit_kills() {
    let mut a = unit("a", 100.0);
    a.actions = vec![action(
        "bound",
        vec![
            damage(1000.0, Target::AbilityTarget),
            damage(1000.0, Target::AbilityTarget),
        ],
    )];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
    s.max_actions = 1;
    let r = run(s);
    close(state(&r, "e1").hp, 0.0);
    close(state(&r, "e2").hp, 1000.0);
    assert_eq!(r.events.iter().filter(|e| e.kind == "damage").count(), 1);
}

#[test]
fn cycle_end_damage_settles_victory_at_the_shared_boundary() {
    let mut s = scenario(vec![unit("a", 1.0)], vec![vec![unit("e", 1.0)]]);
    s.max_av = 30.0;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::CycleEnd,
        vec![damage(1000.0, Target::AllEnemies)],
    )];
    let r = run(s);
    assert_eq!(r.outcome, Outcome::Victory);
    close(r.elapsed_av, 30.0);
}

#[test]
fn extra_turn_entry_death_invalidates_request_and_settles_defeat() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 1.0)]]);
    s.mechanics.triggers = vec![
        hook(
            "a",
            BattleEvent::BattleStart,
            vec![request(RequestKind::ExtraTurn, None)],
        ),
        hook(
            "e",
            BattleEvent::TurnStart,
            vec![damage(1000.0, Target::AllEnemies)],
        ),
    ];
    let r = run(s);
    assert_eq!(r.outcome, Outcome::Defeat);
    assert_eq!(r.actions, 0);
    close(r.elapsed_av, 0.0);
}

#[test]
fn invalidated_queued_ultimate_does_not_pay_and_priority_is_explicit() {
    let mut a = unit("a", 100.0);
    a.initial_energy = 100.0;
    let mut ult = action("u", vec![damage(1.0, Target::AllEnemies)]);
    ult.kind = ActionKind::Ultimate;
    ult.energy_cost = 100.0;
    a.actions.insert(0, ult);
    let mut s = scenario(vec![a], vec![vec![unit("e", 1.0)]]);
    s.max_actions = 1;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::BattleStart,
        vec![
            request(RequestKind::Ultimate, Some("u")),
            Effect::Energy {
                amount: -100.0,
                target: Target::SelfUnit,
            },
        ],
    )];
    let r = run(s);
    assert!(r.events.iter().any(|e| e.kind == "request_invalid"));
    assert!(!r.events.iter().any(|e| e.kind == "ultimate_turn_start"));
    close(state(&r, "a").energy, 0.0);
}

#[test]
fn runtime_listeners_continue_after_trace_limit() {
    let mut a = unit("a", 100.0);
    a.max_energy = 1000000.0;
    let mut s = scenario(vec![a], vec![vec![unit("e", 0.001)]]);
    s.max_actions = 16000;
    s.max_av = 2000000.0;
    let mut t = hook(
        "a",
        BattleEvent::ActionEnd,
        vec![Effect::Energy {
            amount: 1.0,
            target: Target::SelfUnit,
        }],
    );
    t.uses = 0;
    s.mechanics.triggers = vec![t];
    let checked = s;
    let noisy = simulate(&checked, 7, true).unwrap();
    let quiet = simulate(&checked, 7, false).unwrap();
    assert!(noisy.trace_truncated);
    close(state(&noisy, "a").energy, 16000.0);
    close(state(&quiet, "a").energy, 16000.0);
}

#[test]
fn automatic_priority_direction_is_explicit_and_rechecks_after_each_action() {
    for larger in [false, true] {
        let mut a = unit("a", 100.0);
        a.initial_energy = 100.0;
        let mut ult = action("u", vec![damage(1.0, Target::AllEnemies)]);
        ult.kind = ActionKind::Ultimate;
        ult.energy_cost = 100.0;
        a.actions.insert(0, ult);
        let mut b = a.clone();
        b.id = "b".into();
        let mut s = scenario(vec![a, b], vec![vec![unit("e", 1.0)]]);
        s.max_actions = 2;
        s.mechanics.ultimate_priority.insert("a".into(), 10);
        s.mechanics.ultimate_priority.insert("b".into(), 50);
        s.mechanics.larger_priority_first = larger;
        let r = run(s);
        let actors: Vec<_> = r
            .events
            .iter()
            .filter(|e| e.kind == "action")
            .map(|e| e.actor.as_str())
            .collect();
        assert_eq!(
            actors,
            if larger {
                vec!["b", "a"]
            } else {
                vec!["a", "b"]
            }
        );
        close(r.elapsed_av, 0.0);
    }
}

#[test]
fn unit_first_cycle_tie_drains_both_due_units_before_cycle_end() {
    let mut s = scenario(vec![unit("a", 100.0)], vec![vec![unit("e", 100.0)]]);
    s.max_av = 100.0;
    s.mechanics.unit_before_cycle = true;
    let r = run(s);
    assert_eq!(r.actions, 2);
    assert_eq!(r.termination, Termination::AvBudgetExhausted);
    let kinds: Vec<_> = r
        .events
        .iter()
        .filter(|e| e.kind == "action" || e.kind == "cycle_end")
        .map(|e| e.kind.as_str())
        .collect();
    assert_eq!(kinds, vec!["action", "action", "cycle_end"]);
}

#[test]
fn death_listener_queues_followup_after_the_remaining_effects() {
    let mut a = unit("a", 100.0);
    let mut fua = action("fua", vec![damage(7.0, Target::AllEnemies)]);
    fua.kind = ActionKind::FollowUp;
    a.actions = vec![
        action(
            "kill",
            vec![
                damage(1000.0, Target::AbilityTarget),
                Effect::Energy {
                    amount: 5.0,
                    target: Target::SelfUnit,
                },
            ],
        ),
        fua,
    ];
    let mut s = scenario(vec![a], vec![vec![unit("e1", 1.0), unit("e2", 1.0)]]);
    s.max_actions = 2;
    s.mechanics.triggers = vec![hook(
        "a",
        BattleEvent::Death,
        vec![request(RequestKind::FollowUp, Some("fua"))],
    )];
    let r = run(s);
    close(state(&r, "e2").hp, 993.0);
    close(state(&r, "a").energy, 5.0);
    let end = r
        .events
        .iter()
        .position(|e| e.kind == "action_end" && e.action.as_deref() == Some("kill"))
        .unwrap();
    let follow = r
        .events
        .iter()
        .position(|e| e.kind == "action" && e.action.as_deref() == Some("fua"))
        .unwrap();
    assert!(end < follow);
}
