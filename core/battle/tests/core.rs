//! 默认构建的简短主流程检查，保持正式入口独立于诊断功能。
mod common;
use battle_core::{
    batch::{run_batch, BatchRequest},
    engine::{simulate, simulate_cancellable, BattleSession, BattleStep},
    model::*,
};
use common::*;

/// 提供局面层已经选好阵容与技能的最小战斗配置。
fn prepared() -> Scenario {
    let mut hero = unit("hero", 100.0);
    hero.actions = vec![action("strike", vec![damage(1000.0, Target::AllEnemies)])];
    scenario(vec![hero], vec![vec![unit("enemy", 10.0)]])
}

#[test]
fn prepared_input_executes_through_the_production_api() {
    let input = prepared();
    let result = simulate(&input, 7).unwrap();
    assert_eq!((result.outcome, result.actions), (Outcome::Victory, 1));
    close(result.elapsed_av, 100.0);
    close(input.allies[0].initial_hp, 1000.0);
}

#[test]
fn sessions_keep_the_same_public_api_with_or_without_diagnostics() {
    let input = prepared();
    let mut session = BattleSession::new(&input, 7).unwrap();
    loop {
        let (id, choice) = match session.advance().unwrap() {
            BattleStep::Decision(d) => (d.id, d.legal.first().cloned()),
            BattleStep::Finished(r) => {
                assert_eq!((r.outcome, r.actions), (Outcome::Victory, 1));
                break;
            }
        };
        session.submit(id, choice).unwrap();
        assert!(session.submit(id, None).is_err());
    }
}

#[test]
fn cancellation_and_batch_frequencies_keep_their_runtime_meaning() {
    let input = prepared();
    let cancelled = simulate_cancellable(&input, 7, &mut || true).unwrap();
    assert_eq!(
        (cancelled.outcome, cancelled.termination),
        (Outcome::Incomplete, Termination::Cancelled)
    );
    let batch = run_batch(&input, &BatchRequest { trials: 3, seed: 7 }, &mut || false).unwrap();
    assert_eq!(
        (batch.completed, batch.wins, batch.win_rate),
        (3, 3, Some(1.0))
    );
}
