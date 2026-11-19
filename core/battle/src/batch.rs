//! Reproducible serial trials, with incomplete trials kept out of probabilities.
use crate::engine::simulate_cancellable;
use crate::model::{Outcome, Scenario, Termination};
#[cfg(feature = "diagnostics")]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 单次批量评估可启动的最大场数。
pub const MAX_TRIALS: u32 = 1_000_000;
// 保留种子派生域，保证既有批次种子在包名调整后仍能回放。
/// 批次种子派生的固定用途域，保持历史随机序列。
const SEED_DOMAIN: &[u8] = b"hcwa-battle-core/batch-seed/v1\0";

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 独立模拟的场数与主种子。
pub struct BatchRequest {
    /// 本批次请求运行的场数。
    pub trials: u32,
    /// A decimal string on the JSON wire, preserving all 64 bits in JS clients.
    #[cfg_attr(
        feature = "diagnostics",
        serde(with = "crate::diagnostics::seed_decimal")
    )]
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "diagnostics", derive(Serialize))]
/// 频率的双侧 95% Wilson 区间。
pub struct WilsonInterval {
    /// 95% Wilson 区间下界。
    pub lower: f64,
    /// 95% Wilson 区间上界。
    pub upper: f64,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(Serialize))]
#[cfg(feature = "diagnostics")]
/// 仅供诊断记录和按种子重放使用的逐场摘要。
pub struct TrialSummary {
    /// Zero-based index, preserved when replaying a single trial.
    pub index: u32,
    /// JSON uses a decimal string; Rust callers retain the u64 API.
    #[cfg_attr(
        feature = "diagnostics",
        serde(serialize_with = "crate::diagnostics::seed_decimal::serialize")
    )]
    pub seed: u64,
    /// 战斗胜负或未完成状态。
    pub outcome: Outcome,
    /// 结束的具体触发条件。
    pub termination: Termination,
    /// 已实际推进的 AV，独立于罚时。
    pub elapsed_av: f64,
    /// 当前及后续轮次剩余 AV 的合计。
    pub remaining_av: f64,
    /// 实际扣除的复活罚时合计。
    pub penalty_av: f64,
    /// 本场已执行的完整 Action 数。
    pub actions: u32,
    /// 已经清空的波次数。
    pub waves_cleared: usize,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(Serialize))]
/// 按完成样本汇总的胜负频率及停止情况。
pub struct BatchResult {
    #[cfg_attr(
        feature = "diagnostics",
        serde(serialize_with = "crate::diagnostics::seed_decimal::serialize")
    )]
    /// 本批次派生各场种子的起点。
    pub master_seed: u64,
    /// Requested trials; total = started + unstarted.
    pub total: u32,
    /// 已实际启动的场数。
    pub started: u32,
    /// Completed = wins + losses + draws, excluding all incomplete trials.
    pub completed: u32,
    /// 胜利场数。
    pub wins: u32,
    /// 失败场数。
    pub losses: u32,
    /// 平局场数。
    pub draws: u32,
    /// 因取消或保护上限未完成的场数。
    pub incomplete: u32,
    /// A subset of incomplete: trials actually started then cancelled.
    pub cancelled: u32,
    /// 尚未启动的场数。
    pub unstarted: u32,
    /// Also true for cancellation between trials, when cancelled can be zero.
    pub cancellation_requested: bool,
    /// 胜败频率采用的完成场数。
    pub probability_denominator: u32,
    /// 已完成样本中的胜利频率，无样本为 None。
    pub win_rate: Option<f64>,
    /// 已完成样本中的失败频率，无样本为 None。
    pub loss_rate: Option<f64>,
    /// 区间置信水平，当前为 0.95。
    pub confidence_level: f64,
    /// 胜利频率的 95% Wilson 区间。
    pub win_rate_ci95: Option<WilsonInterval>,
    /// 失败频率的 95% Wilson 区间。
    pub loss_rate_ci95: Option<WilsonInterval>,
    #[cfg(feature = "diagnostics")]
    /// 诊断构建保存的逐场摘要。
    pub ledger: Vec<TrialSummary>,
}

/// 按主种子和场次索引派生独立种子，各场不消费其他场的随机流。
///
/// 候选配置可以共用主种子进行比较；各场实际创建独立引擎状态。
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
/// [`BatchRequest::trials`] must be in `1..=MAX_TRIALS` (currently 1,000,000);
/// values outside that range return `Err` before any trial or cancellation check.
pub fn run_batch(
    input: &Scenario,
    request: &BatchRequest,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BatchResult, String> {
    if request.trials == 0 || request.trials > MAX_TRIALS {
        return Err(format!("trials must be 1..{MAX_TRIALS}"));
    }
    let mut output = BatchResult {
        master_seed: request.seed,
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
        #[cfg(feature = "diagnostics")]
        ledger: Vec::new(),
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
        let result = simulate_cancellable(input, seed, &mut sticky_cancel)
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
        #[cfg(feature = "diagnostics")]
        output.ledger.push(TrialSummary {
            index,
            seed,
            outcome: result.outcome,
            termination: result.termination,
            elapsed_av: result.elapsed_av,
            remaining_av: result.remaining_av,
            penalty_av: result.penalty_av,
            actions: result.actions,
            waves_cleared: result.waves_cleared,
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
