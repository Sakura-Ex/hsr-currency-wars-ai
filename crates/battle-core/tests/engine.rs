mod common;

use common::*;
use hcwa_battle_core::{
    engine::{simulate, simulate_cancellable},
    model::*,
};

fn run(input: Scenario) -> BattleResult {
    simulate(
        &validate(input).expect("valid artificial scenario"),
        42,
        true,
    )
    .expect("finite artificial battle")
}

fn actions(result: &BattleResult) -> Vec<(&str, &str, f64)> {
    result
        .events
        .iter()
        .filter(|event| event.kind == "action")
        .map(|event| {
            (
                event.actor.as_str(),
                event.action.as_deref().expect("action id"),
                event.av,
            )
        })
        .collect()
}

fn shield(amount: f64, cap: f64, duration: u16) -> Effect {
    Effect::Shield {
        scale: Scale::Attack,
        ratio: 0.0,
        flat: amount,
        max_total: cap,
        duration_turns: duration,
        target: Target::SelfUnit,
    }
}

#[test]
fn speed_queue_and_equal_time_ties_follow_input_order() {
    let mut input = scenario(
        vec![unit("fast", 200.0), unit("slow", 100.0)],
        vec![vec![unit("enemy", 50.0)]],
    );
    input.max_av = 201.0;
    let result = run(input);
    assert_eq!(
        actions(&result),
        vec![
            ("fast", "wait", 50.0),
            ("fast", "wait", 100.0),
            ("slow", "wait", 100.0),
            ("fast", "wait", 150.0),
            ("fast", "wait", 200.0),
            ("slow", "wait", 200.0),
            ("enemy", "wait", 200.0),
        ]
    );
    assert_eq!(result.actions, 7);
    assert!(result
        .events
        .windows(2)
        .all(|pair| pair[0].av <= pair[1].av));
    assert_eq!(result.run_hp_loss, None);
}

#[test]
fn an_action_exactly_at_the_deadline_is_not_executed() {
    let mut ally = unit("ally", 100.0);
    ally.actions = vec![action("lethal", vec![damage(2000.0, Target::AllEnemies)])];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 100.0)]]);
    input.max_av = 100.0;
    let result = run(input);
    assert_eq!(result.actions, 0);
    assert!(actions(&result).is_empty());
    close(state(&result, "enemy").hp, 1000.0);
    assert_eq!(result.outcome, Outcome::Defeat);
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
}

#[test]
fn terminal_elimination_settles_before_action_limit() {
    let mut ally = unit("ally", 100.0);
    ally.actions = vec![action(
        "lethal",
        vec![damage(2000.0, Target::EnemyLowestHp)],
    )];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 100.0)]]);
    input.max_actions = 1;
    let result = run(input);
    assert_eq!(result.outcome, Outcome::Victory);
    assert_eq!(result.termination, Termination::EnemiesDefeated);
    close(state(&result, "enemy").hp, 0.0);
    assert_eq!(result.actions, 1);
}

#[test]
fn critical_probability_endpoints_have_hand_computed_damage() {
    for (rate, expected_hp, expected_critical) in [(0.0, 990.0, false), (1.0, 980.0, true)] {
        let mut ally = unit("ally", 100.0);
        ally.stats.crit_rate = rate;
        ally.actions = vec![action(
            "hit",
            vec![Effect::Damage {
                scale: Scale::Attack,
                ratio: 1.0,
                flat: 0.0,
                hits: 1,
                can_crit: true,
                target: Target::EnemyLowestHp,
            }],
        )];
        let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
        input.max_av = 101.0;
        let result = run(input);
        close(state(&result, "enemy").hp, expected_hp);
        let hits: Vec<_> = result
            .events
            .iter()
            .filter(|e| e.kind == "damage")
            .collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].critical, Some(expected_critical));
        close(hits[0].amount.expect("damage amount"), 1000.0 - expected_hp);
    }
}

#[test]
fn priority_chooses_affordable_skill_and_pays_before_gaining() {
    let mut ally = unit("ally", 100.0);
    let mut skill = action("skill", vec![damage(30.0, Target::EnemyLowestHp)]);
    skill.kind = ActionKind::Skill;
    skill.sp_cost = 2;
    let mut basic = action("basic", vec![damage(10.0, Target::EnemyLowestHp)]);
    basic.sp_gain = 1;
    ally.actions = vec![skill, basic];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
    input.ally_skill_points.initial = 1;
    input.enemy_skill_points.initial = 4;
    input.max_av = 301.0;
    let result = run(input);
    assert_eq!(
        actions(&result),
        vec![
            ("ally", "basic", 100.0),
            ("ally", "skill", 200.0),
            ("ally", "basic", 300.0)
        ]
    );
    close(state(&result, "enemy").hp, 950.0);
    assert_eq!(result.ally_skill_points, 1);
    assert_eq!(result.enemy_skill_points, 4);
}

#[test]
fn automatic_ultimates_do_not_consume_the_next_normal_turn() {
    let mut ally = unit("ally", 100.0);
    let mut ult = action("ult", vec![damage(5.0, Target::EnemyLowestHp)]);
    ult.kind = ActionKind::Ultimate;
    ult.energy_cost = 100.0;
    let mut basic = action("basic", vec![damage(10.0, Target::EnemyLowestHp)]);
    basic.energy_gain = 100.0;
    ally.actions = vec![ult, basic];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
    input.max_av = 201.0;
    let result = run(input);
    assert_eq!(
        actions(&result),
        vec![
            ("ally", "basic", 100.0),
            ("ally", "ult", 100.0),
            ("ally", "basic", 200.0),
            ("ally", "ult", 200.0)
        ]
    );
    close(state(&result, "enemy").hp, 970.0);
    close(state(&result, "ally").energy, 0.0);
    assert_eq!(result.actions, 4);
}

#[test]
fn health_shields_energy_and_skill_points_are_capped() {
    let mut ally = unit("ally", 100.0);
    ally.initial_hp = 20.0;
    ally.initial_energy = 90.0;
    let mut support = action(
        "support",
        vec![
            Effect::Heal {
                scale: Scale::MaxHp,
                ratio: 0.0,
                flat: 2000.0,
                target: Target::SelfUnit,
            },
            shield(80.0, 100.0, 2),
            shield(80.0, 100.0, 2),
            Effect::Energy {
                amount: 1000.0,
                target: Target::SelfUnit,
            },
        ],
    );
    support.sp_gain = 2;
    support.energy_gain = 100.0;
    ally.actions = vec![support];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
    input.ally_skill_points.initial = 4;
    input.max_av = 101.0;
    let result = run(input);
    close(state(&result, "ally").hp, 1000.0);
    close(state(&result, "ally").shield, 100.0);
    close(state(&result, "ally").energy, 100.0);
    assert_eq!(result.ally_skill_points, 5);
}

#[test]
fn shield_absorbs_before_hp_and_expires_at_recipient_turn_start() {
    for (duration, expected_hp) in [(1, 960.0), (2, 985.0)] {
        let mut ally = unit("ally", 100.0);
        let mut cast = action("shield", vec![shield(25.0, 25.0, duration)]);
        cast.kind = ActionKind::Skill;
        cast.sp_cost = 1;
        ally.actions = vec![cast, idle()];
        let mut enemy = unit("enemy", 50.0);
        enemy.actions = vec![action("hit", vec![damage(40.0, Target::EnemyLowestHp)])];
        let mut input = scenario(vec![ally], vec![vec![enemy]]);
        input.ally_skill_points.initial = 1;
        input.max_av = 201.0;
        let result = run(input);
        close(state(&result, "ally").hp, expected_hp);
        close(state(&result, "ally").shield, 0.0);
    }
}

#[test]
fn self_advance_and_delay_modify_the_already_scheduled_next_turn() {
    for (fraction, expected_times) in [
        (0.5, vec![100.0, 150.0, 200.0, 250.0]),
        (-0.5, vec![100.0, 250.0]),
    ] {
        let mut ally = unit("ally", 100.0);
        ally.actions = vec![action(
            "shift",
            vec![Effect::Advance {
                fraction,
                target: Target::SelfUnit,
            }],
        )];
        let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
        input.max_av = 251.0;
        let result = run(input);
        let actual: Vec<_> = actions(&result).into_iter().map(|(_, _, av)| av).collect();
        assert_eq!(actual, expected_times);
    }
}

fn targeting_scenario(can_crit: bool) -> Scenario {
    let low = unit("low_weight", 1.0);
    let mut high = unit("high_weight", 1.0);
    high.stats.taunt = 3.0;
    let mut enemy = unit("sampler", 100.0);
    enemy.stats.crit_rate = 0.5;
    enemy.actions = vec![action(
        "sample",
        vec![Effect::Damage {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 0.0,
            hits: 100,
            can_crit,
            target: Target::EnemyRandomTaunt,
        }],
    )];
    let mut input = scenario(vec![low, high], vec![vec![enemy]]);
    input.max_actions = 20;
    input.max_av = 2500.0;
    input
}

fn hit_targets(result: &BattleResult) -> Vec<&str> {
    result
        .events
        .iter()
        .filter(|event| event.kind == "damage")
        .map(|event| event.target.as_deref().expect("damage target"))
        .collect()
}

#[test]
fn weighted_targets_obey_support_and_a_predeclared_probability_band() {
    // For weights 1:3, P(high)=0.75. This fixed wide smoke-test band rejects
    // a uniform sampler; it is not a precision claim or real-game validation.
    let result = run(targeting_scenario(false));
    let targets = hit_targets(&result);
    assert_eq!(targets.len(), 2000);
    assert!(targets
        .iter()
        .all(|id| matches!(*id, "low_weight" | "high_weight")));
    let frequency = targets.iter().filter(|id| **id == "high_weight").count() as f64 / 2000.0;
    assert!(
        (0.70..=0.80).contains(&frequency),
        "observed high-weight frequency {frequency}"
    );
    assert!(!(0.70..=0.80).contains(&0.5));
    close(state(&result, "low_weight").hp, 1000.0);
    close(state(&result, "high_weight").hp, 1000.0);
}

#[test]
fn target_stream_is_independent_of_critical_sampling() {
    let without_crits = run(targeting_scenario(false));
    let with_crits = run(targeting_scenario(true));
    assert_eq!(hit_targets(&without_crits), hit_targets(&with_crits));
}

#[test]
fn multihit_random_damage_retargets_only_living_enemies() {
    let mut ally = unit("ally", 100.0);
    ally.actions = vec![action(
        "hit",
        vec![Effect::Damage {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 10.0,
            hits: 100,
            can_crit: false,
            target: Target::EnemyRandomTaunt,
        }],
    )];
    let mut first = unit("first", 1.0);
    let mut second = unit("second", 1.0);
    first.initial_hp = 10.0;
    second.initial_hp = 10.0;
    first.stats.taunt = 1e6;
    let result = run(scenario(vec![ally], vec![vec![first, second]]));
    let hits = hit_targets(&result);
    assert_eq!(hits.len(), 2);
    assert_ne!(hits[0], hits[1]);
    assert_eq!(result.outcome, Outcome::Victory);
}

#[test]
fn waves_preserve_the_global_av_budget_and_ally_schedule() {
    for (budget, expected_outcome, expected_cleared, expected_actions) in [
        (150.0, Outcome::Defeat, 1, 1),
        (200.0, Outcome::Defeat, 1, 1),
        (201.0, Outcome::Victory, 2, 2),
    ] {
        let mut ally = unit("ally", 100.0);
        ally.actions = vec![action("hit", vec![damage(10.0, Target::EnemyLowestHp)])];
        let mut first = unit("first", 50.0);
        let mut second = unit("second", 50.0);
        first.initial_hp = 10.0;
        second.initial_hp = 10.0;
        let mut input = scenario(vec![ally], vec![vec![first], vec![second]]);
        input.max_av = budget;
        let result = run(input);
        assert_eq!(result.outcome, expected_outcome);
        assert_eq!(result.waves_cleared, expected_cleared);
        assert_eq!(result.actions, expected_actions);
        if expected_outcome == Outcome::Victory {
            close(result.elapsed_av, 200.0);
        }
    }
}

#[test]
fn revival_charges_and_penalties_use_a_separate_clock() {
    let mut ally = unit("ally", 100.0);
    ally.stats.max_hp = 10.0;
    ally.initial_hp = 10.0;
    ally.initial_energy = 70.0;
    ally.revival = Some(Revival {
        charges: 2,
        hp_fraction: 1.0,
        av_penalty: 30.0,
    });
    let mut enemy = unit("enemy", 200.0);
    enemy.actions = vec![action("lethal", vec![damage(10.0, Target::EnemyLowestHp)])];
    let result = run(scenario(vec![ally], vec![vec![enemy]]));
    assert_eq!(
        actions(&result),
        vec![
            ("enemy", "lethal", 50.0),
            ("enemy", "lethal", 100.0),
            ("enemy", "lethal", 150.0)
        ]
    );
    close(result.elapsed_av, 150.0);
    close(result.penalty_av, 60.0);
    close(state(&result, "ally").hp, 0.0);
    close(state(&result, "ally").energy, 70.0);
    assert_eq!(state(&result, "ally").revivals_left, 0);
    assert_eq!(result.outcome, Outcome::Defeat);
    assert_eq!(result.termination, Termination::AlliesDefeated);
}

#[test]
fn revival_penalty_can_exhaust_budget_without_advancing_natural_time() {
    let mut ally = unit("ally", 100.0);
    ally.stats.max_hp = 10.0;
    ally.initial_hp = 10.0;
    ally.revival = Some(Revival {
        charges: 1,
        hp_fraction: 0.5,
        av_penalty: 50.0,
    });
    let mut enemy = unit("enemy", 200.0);
    enemy.actions = vec![action("lethal", vec![damage(10.0, Target::EnemyLowestHp)])];
    let mut input = scenario(vec![ally], vec![vec![enemy]]);
    input.max_av = 100.0;
    let result = run(input);
    close(result.elapsed_av, 50.0);
    close(result.penalty_av, 50.0);
    close(state(&result, "ally").hp, 5.0);
    assert_eq!(result.actions, 1);
    assert_eq!(result.outcome, Outcome::Defeat);
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
}

#[test]
fn action_limit_and_cancellation_are_incomplete_not_defeats() {
    let mut input = scenario(vec![unit("ally", 100.0)], vec![vec![unit("enemy", 100.0)]]);
    input.max_actions = 1;
    let validated = validate(input).unwrap();
    let limited = simulate(&validated, 42, true).unwrap();
    assert_eq!(limited.outcome, Outcome::Incomplete);
    assert_eq!(limited.termination, Termination::ActionLimit);
    assert_eq!(limited.actions, 1);
    let cancelled = simulate_cancellable(&validated, 42, true, &mut || true).unwrap();
    assert_eq!(cancelled.outcome, Outcome::Incomplete);
    assert_eq!(cancelled.termination, Termination::Cancelled);
    assert_eq!(cancelled.actions, 0);
}

#[test]
fn same_seed_replays_the_full_result_and_trace_does_not_change_state() {
    let validated = validate(targeting_scenario(true)).unwrap();
    for seed in [0, 1, 42, u64::MAX] {
        let first = simulate(&validated, seed, true).unwrap();
        let second = simulate(&validated, seed, true).unwrap();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
        let mut traced = serde_json::to_value(&first).unwrap();
        let quiet = simulate(&validated, seed, false).unwrap();
        assert!(quiet.events.is_empty());
        traced["events"] = serde_json::json!([]);
        assert_eq!(traced, serde_json::to_value(quiet).unwrap());
        assert_eq!(first.seed, seed);
        assert_eq!(first.scenario_sha256, validated.fingerprint());
    }
    assert_ne!(
        hit_targets(&simulate(&validated, 0, true).unwrap()),
        hit_targets(&simulate(&validated, 1, true).unwrap())
    );
}

#[test]
fn illegal_and_explicitly_unsupported_scenarios_are_rejected() {
    let base = scenario(vec![unit("ally", 100.0)], vec![vec![unit("enemy", 100.0)]]);
    type RejectedInputCase = (&'static str, fn(&mut Scenario));
    let changes: Vec<RejectedInputCase> = vec![
        ("zero speed", |s| s.allies[0].stats.speed = 0.0),
        ("nonfinite speed", |s| s.allies[0].stats.speed = f64::NAN),
        ("invalid critical probability", |s| {
            s.allies[0].stats.crit_rate = 1.01
        }),
        ("nonfinite deadline", |s| s.max_av = f64::INFINITY),
        ("empty wave", |s| s.waves[0].clear()),
        ("ambiguous unit identity", |s| {
            s.waves[0][0].id = "ally".into()
        }),
        ("resource overflow", |s| s.allies[0].initial_energy = 101.0),
        ("illegal action cost", |s| {
            s.allies[0].actions[0].sp_cost = 6
        }),
        ("missing free fallback", |s| {
            s.allies[0].actions[0].energy_cost = 1.0
        }),
        ("free ultimate", |s| {
            s.allies[0].actions[0].kind = ActionKind::Ultimate
        }),
        ("unsupported profile", |s| s.ruleset = "full_game".into()),
        ("declared unsupported mechanic", |s| {
            s.unsupported_mechanics.push("summon".into())
        }),
        ("damage to ally", |s| {
            s.allies[0].actions[0].effects = vec![damage(10.0, Target::SelfUnit)]
        }),
    ];
    for (label, change) in changes {
        let mut invalid = base.clone();
        change(&mut invalid);
        assert!(validate(invalid).is_err(), "accepted {label}");
    }
}

#[test]
fn unknown_fields_and_effects_are_not_silently_dropped() {
    let base = scenario(vec![unit("ally", 100.0)], vec![vec![unit("enemy", 100.0)]]);
    let mut unknown_field = serde_json::to_value(&base).unwrap();
    unknown_field["hidden_buff"] = serde_json::json!(10);
    assert!(serde_json::from_value::<Scenario>(unknown_field).is_err());
    let mut unknown_effect = serde_json::to_value(&base).unwrap();
    unknown_effect["allies"][0]["actions"][0]["effects"][0] =
        serde_json::json!({"type": "summon", "id": "extra"});
    assert!(serde_json::from_value::<Scenario>(unknown_effect).is_err());
}

#[test]
fn valid_over_reduction_and_negative_penetration_reach_settlement() {
    let mut ally = unit("ally", 100.0);
    ally.stats.defense_reduction = 1.1;
    ally.stats.defense_ignore = 1.1;
    ally.stats.resistance_penetration = -0.1;
    ally.actions = vec![action("hit", vec![damage(10.0, Target::EnemyLowestHp)])];
    let mut enemy = unit("enemy", 1.0);
    enemy.stats.defense = 1000.0;
    let mut input = scenario(vec![ally], vec![vec![enemy]]);
    input.max_av = 101.0;
    // Effective defense is clamped to zero; resistance multiplier is 0.9.
    close(state(&run(input), "enemy").hp, 991.0);
}

#[test]
fn waves_preserve_team_resources_but_load_new_enemy_initial_energy() {
    let mut ally = unit("ally", 100.0);
    ally.initial_hp = 30.0;
    let mut basic = action("hit", vec![damage(10.0, Target::EnemyLowestHp)]);
    basic.energy_gain = 5.0;
    basic.sp_gain = 1;
    ally.actions = vec![basic];
    let mut skill = action("skill", vec![damage(7.0, Target::EnemyLowestHp)]);
    skill.kind = ActionKind::Skill;
    skill.sp_cost = 2;
    let mut first = unit("first", 200.0);
    first.initial_hp = 10.0;
    first.actions = vec![skill.clone(), idle()];
    let mut second = unit("second", 200.0);
    second.initial_hp = 10.0;
    second.initial_energy = 50.0;
    let mut ultimate = action("ult", vec![damage(3.0, Target::EnemyLowestHp)]);
    ultimate.kind = ActionKind::Ultimate;
    ultimate.energy_cost = 50.0;
    second.actions = vec![ultimate, skill, idle()];
    let mut input = scenario(vec![ally], vec![vec![first], vec![second]]);
    input.enemy_skill_points.initial = 2;
    let result = run(input);
    assert_eq!(
        actions(&result),
        vec![
            ("first", "skill", 50.0),
            ("ally", "hit", 100.0),
            ("second", "ult", 100.0),
            ("second", "wait", 150.0),
            ("ally", "hit", 200.0),
        ]
    );
    assert_eq!(result.outcome, Outcome::Victory);
    close(state(&result, "ally").hp, 20.0);
    close(state(&result, "ally").energy, 10.0);
    assert_eq!(result.ally_skill_points, 2);
    assert_eq!(result.enemy_skill_points, 0);
}

#[test]
fn final_elimination_within_one_action_wins_before_its_penalty_budget_check() {
    let mut ally = unit("ally", 100.0);
    ally.actions = vec![action(
        "two_hits",
        vec![Effect::Damage {
            scale: Scale::Attack,
            ratio: 0.0,
            flat: 10.0,
            hits: 2,
            can_crit: false,
            target: Target::EnemyLowestHp,
        }],
    )];
    let mut enemy = unit("enemy", 1.0);
    enemy.stats.max_hp = 10.0;
    enemy.initial_hp = 10.0;
    enemy.revival = Some(Revival {
        charges: 1,
        hp_fraction: 0.5,
        av_penalty: 1000.0,
    });
    let mut input = scenario(vec![ally], vec![vec![enemy]]);
    input.max_av = 101.0;
    let result = run(input);
    // Hit 1 revives at 5 HP; hit 2 kills. The action settles before budget checking.
    assert_eq!(result.outcome, Outcome::Victory);
    assert_eq!(result.actions, 1);
    close(result.elapsed_av, 100.0);
    close(result.penalty_av, 1000.0);
    close(state(&result, "enemy").hp, 0.0);
}

#[test]
fn immediate_ultimate_chains_obey_both_limit_and_mid_chain_cancellation() {
    let mut ally = unit("ally", 100.0);
    ally.initial_energy = 100.0;
    let mut ultimate = action(
        "refill",
        vec![Effect::Energy {
            amount: 100.0,
            target: Target::SelfUnit,
        }],
    );
    ultimate.kind = ActionKind::Ultimate;
    ultimate.energy_cost = 100.0;
    ally.actions = vec![ultimate, idle()];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 100.0)]]);
    input.max_actions = 10;
    let validated = validate(input).unwrap();
    let limited = simulate(&validated, 42, true).unwrap();
    assert_eq!(limited.outcome, Outcome::Incomplete);
    assert_eq!(limited.termination, Termination::ActionLimit);
    assert_eq!(limited.actions, 10);
    close(limited.elapsed_av, 0.0);
    let mut calls = 0;
    let cancelled = simulate_cancellable(&validated, 42, true, &mut || {
        calls += 1;
        calls == 3
    })
    .unwrap();
    assert_eq!(cancelled.outcome, Outcome::Incomplete);
    assert_eq!(cancelled.termination, Termination::Cancelled);
    assert_eq!(cancelled.actions, 2);
    close(cancelled.elapsed_av, 0.0);
}
