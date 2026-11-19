//! Explicit provisional business settlement, separate from combat outcome.
use crate::model::Outcome;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// 开发场景使用的临时加权击败进度与 HP 结算规则。
pub struct SettlementPolicy {
    /// 临时结算策略的名称。
    pub name: String,
    /// 每个敌人实例贡献的正进度权重。
    pub enemy_weights: BTreeMap<String, f64>,
    /// 免除罚血所需的最低击败进度。
    pub minimum_progress: f64,
    /// 临时结算前的整局 HP。
    pub hp_before: f64,
    /// 整局 HP 上限。
    pub hp_cap: f64,
    /// 进度不足时请求扣除的 HP。
    pub failure_loss: f64,
    /// 扣血后请求恢复的 HP。
    pub recovery: f64,
}

#[derive(Clone, Debug, Serialize)]
/// 开发工具侧临时结算的进度与实际 HP 变化。
pub struct SettlementResult {
    /// 结算已应用或因战斗未完成而未应用的状态。
    pub status: String,
    /// 本次使用的完整临时结算规则。
    pub policy: SettlementPolicy,
    /// 已经入场且最终死亡的敌人 ID。
    pub defeated_ids: Vec<String>,
    /// 已击败敌人的权重合计。
    pub defeated_weight: f64,
    /// 全部敌人权重合计，包含尚未入场者。
    pub total_weight: f64,
    /// 已击败权重除以总权重。
    pub progress: f64,
    /// 当前进度是否达到免罚血阈值。
    pub minimum_progress_met: bool,
    /// 完成战斗请求的扣血量，未完成时为 None。
    pub requested_loss: Option<f64>,
    /// 按当前 HP 裁剪后的实际扣血量。
    pub actual_loss: Option<f64>,
    /// 完成战斗请求的恢复量。
    pub requested_recovery: Option<f64>,
    /// 按 HP 上限裁剪后的实际恢复量。
    pub actual_recovery: Option<f64>,
    /// 因当前 HP 不足而裁剪掉的扣血量。
    pub loss_clipped: Option<f64>,
    /// 因 HP 上限而裁剪掉的恢复量。
    pub recovery_clipped: Option<f64>,
    /// 应用临时结算后的 HP，未完成时为 None。
    pub hp_after: Option<f64>,
    /// 临时结算后 HP 是否仍大于零。
    pub run_alive: Option<bool>,
}

/// 开发报告生成时计算一次临时结算；未完成战斗保留未应用状态。
pub(crate) fn settle(
    policy: &SettlementPolicy,
    defeated_ids: Vec<String>,
    outcome: Outcome,
) -> SettlementResult {
    let total_weight = policy.enemy_weights.values().sum::<f64>();
    // Use the same key order as the denominator, including when input queue order differs.
    let defeated = defeated_ids
        .iter()
        .collect::<std::collections::BTreeSet<_>>();
    let defeated_weight = policy
        .enemy_weights
        .iter()
        .filter(|(id, _)| defeated.contains(id))
        .map(|(_, weight)| *weight)
        .sum::<f64>();
    let progress = defeated_weight / total_weight;
    let minimum_progress_met = progress >= policy.minimum_progress;
    let mut result = SettlementResult {
        status: "not_applied_incomplete".into(),
        policy: policy.clone(),
        defeated_ids,
        defeated_weight,
        total_weight,
        progress,
        minimum_progress_met,
        requested_loss: None,
        actual_loss: None,
        requested_recovery: None,
        actual_recovery: None,
        loss_clipped: None,
        recovery_clipped: None,
        hp_after: None,
        run_alive: None,
    };
    if outcome != Outcome::Incomplete {
        let loss = if minimum_progress_met {
            0.0
        } else {
            policy.failure_loss
        };
        let actual_loss = loss.min(policy.hp_before);
        let after_loss = policy.hp_before - actual_loss;
        let actual_recovery = policy.recovery.min(policy.hp_cap - after_loss);
        let hp_after = after_loss + actual_recovery;
        result.status = "applied_provisional".into();
        result.requested_loss = Some(loss);
        result.actual_loss = Some(actual_loss);
        result.requested_recovery = Some(policy.recovery);
        result.actual_recovery = Some(actual_recovery);
        result.loss_clipped = Some(loss - actual_loss);
        result.recovery_clipped = Some(policy.recovery - actual_recovery);
        result.hp_after = Some(hp_after);
        result.run_alive = Some(hp_after > 0.0);
    }
    result
}
