//! Reproducible serial trials, with incomplete trials kept out of probabilities.
use crate::engine::{simulate_cancellable, RNG_ALGORITHM};
use crate::model::{Outcome, Termination, ValidatedScenario};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_TRIALS: u32 = 1_000_000;
pub const SEED_DERIVATION: &str =
    "sha256(hcwa-battle-core/batch-seed/v1\\0 || master_u64_le || index_u64_le)[0..8] as u64_le";
const SEED_DOMAIN: &[u8] = b"hcwa-battle-core/batch-seed/v1\0";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    pub trials: u32,
    /// A decimal string on the JSON wire, preserving all 64 bits in JS clients.
    #[serde(with = "crate::model::seed_decimal")]
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct WilsonInterval {
    pub lower: f64,
    pub upper: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TrialSummary {
    /// Zero-based index, preserved when replaying a single trial.
    pub index: u32,
    /// JSON uses a decimal string; Rust callers retain the u64 API.
    #[serde(serialize_with = "crate::model::seed_decimal::serialize")]
    pub seed: u64,
    pub outcome: Outcome,
    pub termination: Termination,
    pub elapsed_av: f64,
    pub penalty_av: f64,
    pub actions: u32,
    pub waves_cleared: usize,
    /// Unknown settlement, never replaced by zero or remaining unit HP.
    pub run_hp_loss: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BatchResult {
    pub scenario_sha256: String,
    pub ruleset: String,
    #[serde(serialize_with = "crate::model::seed_decimal::serialize")]
    pub master_seed: u64,
    pub seed_derivation: String,
    pub rng: String,
    /// Requested trials; total = started + unstarted.
    pub total: u32,
    pub started: u32,
    /// Completed = wins + losses + draws, excluding all incomplete trials.
    pub completed: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub incomplete: u32,
    /// A subset of incomplete: trials actually started then cancelled.
    pub cancelled: u32,
    pub unstarted: u32,
    /// Also true for cancellation between trials, when cancelled can be zero.
    pub cancellation_requested: bool,
    pub probability_denominator: u32,
    pub win_rate: Option<f64>,
    pub loss_rate: Option<f64>,
    pub confidence_level: f64,
    pub win_rate_ci95: Option<WilsonInterval>,
    pub loss_rate_ci95: Option<WilsonInterval>,
    pub run_hp_loss_unknown_count: u32,
    pub run_hp_loss_status: String,
    pub ledger: Vec<TrialSummary>,
    pub validation_scope: String,
}

/// Derive each stream directly by index; no trial consumes the next trial's RNG.
/// The scenario fingerprint is recorded separately, allowing common seeds across
/// candidate scenarios. This is a deterministic stream choice, not a claim of
/// collision-free seeds or validation against the live game's random generator.
pub fn derive_trial_seed(master_seed: u64, index: u64) -> u64 {
    let mut hash = Sha256::new();
    hash.update(SEED_DOMAIN);
    hash.update(master_seed.to_le_bytes());
    hash.update(index.to_le_bytes());
    let digest = hash.finalize();
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    u64::from_le_bytes(first)
}

/// Two-sided 95% Wilson score interval. No observations yields None, not [0, 0].
pub fn wilson_interval(successes: u64, trials: u64) -> Result<Option<WilsonInterval>, String> {
    if successes > trials {
        return Err("Wilson successes cannot exceed trials".into());
    }
    if trials == 0 {
        return Ok(None);
    }
    let n = trials as f64;
    let p = successes as f64 / n;
    let z: f64 = 1.959_963_984_540_054;
    let z_squared = z * z;
    let denominator = 1.0 + z_squared / n;
    let center = (p + z_squared / (2.0 * n)) / denominator;
    let half = z * (p * (1.0 - p) / n + z_squared / (4.0 * n * n)).sqrt() / denominator;
    Ok(Some(WilsonInterval {
        lower: (center - half).clamp(0.0, 1.0),
        upper: (center + half).clamp(0.0, 1.0),
    }))
}

/// Run independent, fresh engine instances serially. Cancellation is sticky,
/// checked both between trials and by the engine during a trial. Engine failures
/// propagate with the trial identity; they never become defeats or valid samples.
pub fn run_batch(
    validated: &ValidatedScenario,
    request: &BatchRequest,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BatchResult, String> {
    if request.trials == 0 || request.trials > MAX_TRIALS {
        return Err(format!("trials must be 1..{MAX_TRIALS}"));
    }
    let mut output = BatchResult {
        scenario_sha256: validated.fingerprint().to_owned(),
        ruleset: validated.scenario().ruleset.clone(),
        master_seed: request.seed,
        seed_derivation: SEED_DERIVATION.into(),
        rng: RNG_ALGORITHM.into(),
        total: request.trials,
        started: 0,
        completed: 0,
        wins: 0,
        losses: 0,
        draws: 0,
        incomplete: 0,
        cancelled: 0,
        unstarted: request.trials,
        cancellation_requested: false,
        probability_denominator: 0,
        win_rate: None,
        loss_rate: None,
        confidence_level: 0.95,
        win_rate_ci95: None,
        loss_rate_ci95: None,
        run_hp_loss_unknown_count: 0,
        run_hp_loss_status: "unknown: run settlement is outside basic_v1".into(),
        ledger: Vec::new(),
        validation_scope: "Explicit-input basic_v1 Monte Carlo; conditional on completed trials; not full 4.7 roster, live-game win-rate validation, or run HP settlement".into(),
    };
    for index in 0..request.trials {
        if cancel() {
            output.cancellation_requested = true;
            break;
        }
        let seed = derive_trial_seed(request.seed, u64::from(index));
        let mut observed_cancel = false;
        let mut sticky_cancel = || {
            if !observed_cancel {
                observed_cancel = cancel();
            }
            observed_cancel
        };
        let result = simulate_cancellable(validated, seed, false, &mut sticky_cancel)
            .map_err(|error| format!("trial {index}, seed {seed}: {error}"))?;
        // These checks protect the statistical denominator against engine/API
        // drift, especially an action limit or cancellation reported as a loss.
        if matches!(
            result.termination,
            Termination::ActionLimit | Termination::Cancelled
        ) && result.outcome != Outcome::Incomplete
        {
            return Err(format!(
                "trial {index}: safety/cancellation termination must be incomplete"
            ));
        }
        if result.scenario_sha256 != output.scenario_sha256
            || result.ruleset != output.ruleset
            || result.seed != seed
        {
            return Err(format!("trial {index}: engine result identity mismatch"));
        }
        if result.run_hp_loss.is_some() {
            return Err(format!(
                "trial {index}: basic_v1 does not implement run HP settlement"
            ));
        }
        match result.outcome {
            Outcome::Victory => output.wins += 1,
            Outcome::Defeat => output.losses += 1,
            Outcome::Draw => output.draws += 1,
            Outcome::Incomplete => output.incomplete += 1,
        }
        let trial_cancelled = result.termination == Termination::Cancelled;
        if trial_cancelled {
            output.cancelled += 1;
        }
        output.started += 1;
        output.run_hp_loss_unknown_count += 1;
        output.ledger.push(TrialSummary {
            index,
            seed,
            outcome: result.outcome,
            termination: result.termination,
            elapsed_av: result.elapsed_av,
            penalty_av: result.penalty_av,
            actions: result.actions,
            waves_cleared: result.waves_cleared,
            run_hp_loss: None,
        });
        if observed_cancel || trial_cancelled {
            output.cancellation_requested = true;
            break;
        }
    }
    output.completed = output.wins + output.losses + output.draws;
    output.unstarted = output.total - output.started;
    output.probability_denominator = output.completed;
    if output.completed > 0 {
        output.win_rate = Some(f64::from(output.wins) / f64::from(output.completed));
        output.loss_rate = Some(f64::from(output.losses) / f64::from(output.completed));
    }
    output.win_rate_ci95 = wilson_interval(u64::from(output.wins), u64::from(output.completed))?;
    output.loss_rate_ci95 = wilson_interval(u64::from(output.losses), u64::from(output.completed))?;
    Ok(output)
}
