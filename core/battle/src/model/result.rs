//! 单局、单位与回放记录的输出格式。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 一场战斗的胜负或未完成状态。
pub enum Outcome {
    /// 全部敌方波次或有限队列已清空。
    Victory,
    /// 我方全灭或倒计时额度耗尽。
    Defeat,
    /// 双方同时被消灭。
    Draw,
    /// 因程序保护或调用方取消而提前停止。
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 导致战斗返回的具体结束条件。
pub enum Termination {
    /// 全部敌人被击败。
    EnemiesDefeated,
    /// 我方全部被击败。
    AlliesDefeated,
    /// 双方均被击败。
    BothDefeated,
    /// 最终 Cycle 的额度已经结算完毕。
    AvBudgetExhausted,
    /// 达到完整 Action 的程序上限。
    ActionLimit,
    /// 调用方请求取消。
    Cancelled,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 供策略及最终结果共用的当前单位状态。
pub struct UnitResult {
    /// None means no natural-weakness input, not an empty natural weakness set.
    #[cfg_attr(
        feature = "diagnostics",
        serde(skip_serializing_if = "Option::is_none")
    )]
    pub weaknesses: Option<crate::weakness::WeaknessState>,
    /// 所属对象的稳定标识。
    pub id: String,
    /// 是否属于我方。
    pub ally: bool,
    /// 当前生命值。
    pub hp: f64,
    /// 当前护盾容量。
    pub shield: f64,
    /// 当前能量。
    pub energy: f64,
    /// 当前生命值是否大于零。
    pub alive: bool,
    /// 本场剩余即时复活次数。
    pub revivals_left: u8,
    /// 距下次普通行动的剩余距离。
    pub remaining_distance: f64,
    /// 当前或入场速度，等待 AV 等于剩余距离除以速度。
    pub speed: f64,
    /// 单位当前韧性或按单位关联的初始韧性。
    pub toughness: f64,
    /// 单位持久变量表或按单位关联的初始变量。
    pub dynamic_values: std::collections::BTreeMap<String, f64>,
    /// 当前活动属性状态。
    pub modifiers: Vec<crate::modifiers::ModifierState>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 有限敌人队列的槽位与待入场位置。
pub struct QueueResult {
    /// 有限队列同时在场的槽位数。
    pub capacity: usize,
    /// 各槽当前的存活单位 ID，空槽为 None。
    pub active_slots: Vec<Option<String>>,
    /// 尚未入场的敌人 ID，保持队列顺序。
    pub remaining_ids: Vec<String>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 单场执行结果，交给局面运营层完成整局结算。
pub struct BattleResult {
    #[cfg_attr(
        feature = "diagnostics",
        serde(skip_serializing_if = "Option::is_none")
    )]
    /// 有限队列模式下的当前槽位和待入场位置。
    pub queue: Option<QueueResult>,
    /// 战斗胜负或未完成状态。
    pub outcome: Outcome,
    /// 结束的具体触发条件。
    pub termination: Termination,
    /// 已实际推进的 AV，独立于罚时。
    pub elapsed_av: f64,
    /// 当前倒计时轮次，内部从零、对外从一开始。
    pub cycle: usize,
    /// 当前轮剩余 AV。
    pub cycle_remaining_av: f64,
    /// 当前及后续轮次剩余 AV 的合计。
    pub remaining_av: f64,
    /// 实际扣除的复活罚时合计。
    pub penalty_av: f64,
    /// 已执行的完整 Action 数。
    pub actions: u32,
    /// 已经清空的波次数。
    pub waves_cleared: usize,
    /// 我方当前共享战技点。
    pub ally_skill_points: u8,
    /// 敌方当前共享战技点。
    pub enemy_skill_points: u8,
    /// 已经入场的单位，死亡记录保留。
    pub units: Vec<UnitResult>,
    /// 诊断构建专用的顺序回放。
    #[cfg(feature = "diagnostics")]
    pub events: Vec<crate::diagnostics::Event>,
    /// 诊断回放是否达到记录上限。
    #[cfg(feature = "diagnostics")]
    pub trace_truncated: bool,
}
