mod common;
use battle_core::batch::{run_batch, BatchRequest};
use battle_core::diagnostics::{simulate, simulate_cancellable};
use battle_core::model::*;
use common::*;

fn boundary_scenario() -> Scenario {
    let mut ally = unit("ally", 123.45000000000009);
    ally.actions = vec![action("hit", vec![damage(1000.0, Target::EnemyLowestHp)])];
    let mut input = scenario(vec![ally], vec![vec![unit("enemy", 1.0)]]);
    // The exact original action is one representable AV below this deadline.
    input.max_av = 81.00445524503843;
    input
}

#[test]
fn json_roundtrip_preserves_deadline_result() {
    let original = boundary_scenario();
    let restored = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
    let before = simulate(&original, 17, true).unwrap();
    let after = simulate(&restored, 17, true).unwrap();
    assert_eq!(before.outcome, Outcome::Victory);
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[test]
fn json_roundtrip_preserves_neighboring_speed_bits() {
    for offset in 0..256 {
        let mut input = boundary_scenario();
        input.allies[0].stats.speed = f64::from_bits(123.45_f64.to_bits() + offset);
        let restored: Scenario =
            serde_json::from_slice(&serde_json::to_vec(&input).unwrap()).unwrap();
        assert_eq!(
            input.allies[0].stats.speed.to_bits(),
            restored.allies[0].stats.speed.to_bits()
        );
    }
}

fn penalty_scenario(penalty: f64) -> Scenario {
    let mut ally = unit("ally", 200.0);
    ally.actions = vec![action("hit", vec![damage(1000.0, Target::EnemyLowestHp)])];
    let mut enemy = unit("enemy", 100.0);
    enemy.revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: penalty,
    });
    let mut input = scenario(vec![ally], vec![vec![enemy]]);
    input.max_av = 100.0;
    input.max_actions = 1;
    input
}

#[test]
fn exhausted_budget_precedes_action_limit_and_counts_as_loss() {
    for penalty in [50.0, 75.0] {
        let input = penalty_scenario(penalty);
        let result = simulate(&input, 17, true).unwrap();
        println!(
            "penalty={penalty}, outcome={:?}, termination={:?}",
            result.outcome, result.termination
        );
        assert_eq!(result.termination, Termination::AvBudgetExhausted);
        assert_eq!(result.outcome, Outcome::Defeat);
        assert_eq!(result.elapsed_av, 50.0);
        assert_eq!(result.penalty_av, penalty.min(50.0));
        assert_eq!(result.actions, 1);
        let batch = run_batch(
            &input,
            &BatchRequest {
                trials: 3,
                seed: 17,
            },
            &mut || false,
        )
        .unwrap();
        assert_eq!(
            (
                batch.losses,
                batch.completed,
                batch.probability_denominator,
                batch.incomplete
            ),
            (3, 3, 3, 0)
        );
        assert_eq!(batch.loss_rate, Some(1.0));
    }
}

#[test]
fn already_exhausted_budget_precedes_new_cancellation() {
    let input = penalty_scenario(50.0);
    let mut checks = 0;
    let result = simulate_cancellable(&input, 17, false, &mut || {
        checks += 1;
        checks > 1
    })
    .unwrap();
    assert_eq!(result.termination, Termination::AvBudgetExhausted);
    assert_eq!(checks, 1);
}

#[test]
fn unexpired_budget_does_not_forecast_a_loss_past_action_limit() {
    let input = penalty_scenario(49.0);
    let result = simulate(&input, 17, false).unwrap();
    assert_eq!(result.termination, Termination::ActionLimit);
    assert_eq!(result.outcome, Outcome::Incomplete);
    let mut checks = 0;
    let result = simulate_cancellable(&input, 17, false, &mut || {
        checks += 1;
        checks > 1
    })
    .unwrap();
    assert_eq!(result.termination, Termination::Cancelled);
}

#[test]
fn final_elimination_still_precedes_budget_and_action_limit() {
    let mut input = penalty_scenario(50.0);
    input.allies[0].actions[0]
        .effects
        .push(damage(1000.0, Target::EnemyLowestHp));
    let result = simulate(&input, 17, false).unwrap();
    assert_eq!(result.outcome, Outcome::Victory);
    assert_eq!(result.termination, Termination::EnemiesDefeated);
    assert_eq!(result.penalty_av, 50.0);
    assert_eq!(result.actions, 1);
}

fn heal(target: Target) -> Effect {
    Effect::Heal {
        scale: Scale::Attack,
        ratio: 0.0,
        flat: 100.0,
        target,
    }
}

#[test]
fn effective_negative_support_modifiers_execute_with_hand_computed_amounts() {
    let mut actor = unit("actor", 200.0);
    actor.stats.healing_bonus = -0.25;
    actor.stats.shield_bonus = -0.25;
    actor.actions = vec![action(
        "support",
        vec![
            heal(Target::AllyLowestHp),
            Effect::Shield {
                scale: Scale::Attack,
                ratio: 0.0,
                flat: 100.0,
                max_total: 1000.0,
                duration_turns: 3,
                target: Target::AllyLowestHp,
            },
        ],
    )];
    let mut patient = unit("patient", 100.0);
    patient.initial_hp = 100.0;
    patient.stats.incoming_healing_bonus = -0.25;
    let mut input = scenario(vec![actor, patient], vec![vec![unit("enemy", 1.0)]]);
    input.max_actions = 1;
    let result = simulate(&input, 17, true).unwrap();
    assert_eq!(state(&result, "patient").hp, 150.0);
    assert_eq!(state(&result, "patient").shield, 75.0);
}

#[test]
fn completed_loss_is_kept_when_batch_cancels_before_next_trial() {
    let input = penalty_scenario(50.0);
    let mut checks = 0;
    let batch = run_batch(
        &input,
        &BatchRequest {
            trials: 3,
            seed: 17,
        },
        &mut || {
            checks += 1;
            checks == 3
        },
    )
    .unwrap();
    assert_eq!(
        (
            batch.started,
            batch.completed,
            batch.losses,
            batch.incomplete,
            batch.unstarted
        ),
        (1, 1, 1, 0, 2)
    );
    assert!(batch.cancellation_requested);
    assert_eq!(batch.cancelled, 0);
    assert_eq!(batch.probability_denominator, 1);
}

#[test]
fn support_modifiers_accept_zero_effect_and_reject_below_minus_one() {
    let mut actor = unit("actor", 200.0);
    actor.initial_hp = 100.0;
    actor.stats.healing_bonus = -0.5;
    actor.stats.incoming_healing_bonus = -0.5;
    actor.stats.shield_bonus = -1.0;
    actor.actions = vec![action(
        "support",
        vec![
            heal(Target::SelfUnit),
            Effect::Shield {
                scale: Scale::Attack,
                ratio: 0.0,
                flat: 100.0,
                max_total: 1000.0,
                duration_turns: 3,
                target: Target::SelfUnit,
            },
        ],
    )];
    let mut input = scenario(vec![actor], vec![vec![unit("enemy", 1.0)]]);
    input.max_actions = 1;
    let result = simulate(&input.clone(), 17, false).unwrap();
    assert_eq!(state(&result, "actor").hp, 100.0);
    assert_eq!(state(&result, "actor").shield, 0.0);
}

#[test]
fn json_cli_matches_library_at_float_deadline() {
    use std::process::Command;
    let input = boundary_scenario();
    let expected = simulate(&input, 17, true).unwrap();
    let directory = std::env::temp_dir().join(format!("hcwa-roundtrip-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("scenario.json");
    std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hcwa-battle"))
        .arg("simulate")
        .arg(&path)
        .arg("17")
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual["outcome"],
        serde_json::to_value(expected.outcome).unwrap()
    );
    assert_eq!(
        actual["events"],
        serde_json::to_value(expected.events).unwrap()
    );
}
