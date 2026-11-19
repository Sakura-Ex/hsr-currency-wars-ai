//! Automatic and scripted callers select from the same currently legal operations.
use crate::model::*;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 当前合法操作中的执行者与技能索引。
pub struct Operation {
    /// 本场执行者索引或回放中的执行者 ID。
    pub actor: usize,
    /// 可选动作标识或该执行者动作列表中的索引。
    pub action: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 外部策略选择动作所处的窗口。
pub enum DecisionPoint {
    /// 当前可登记终结技或放弃插入的窗口。
    Insert,
    /// 普通单位回合的选招窗口。
    NormalTurn,
    /// 进入额外回合并在回合开始事件后选招。
    ExtraTurn,
}
#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 一名存活单位当前普通行动的位置。
pub struct AxisPosition {
    /// 所属对象的稳定标识。
    pub id: String,
    /// 距下次普通行动的剩余距离。
    pub remaining_distance: f64,
    /// 当前或入场速度，等待 AV 等于剩余距离除以速度。
    pub speed: f64,
    /// 按剩余距离除以当前速度计算的等待 AV。
    pub waiting_av: f64,
}
#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 当前可供策略读取的资源、行动轴和已入场单位状态。
pub struct Observation {
    /// 实际推进的 AV 时钟。
    pub now: f64,
    /// 当前倒计时轮次，内部从零、对外从一开始。
    pub cycle: usize,
    /// 当前轮剩余 AV。
    pub cycle_remaining_av: f64,
    /// 当前Cycle及后续Cycle的剩余总AV，供外部策略分配预算。
    pub remaining_av: f64,
    /// 我方当前共享战技点。
    pub ally_skill_points: u8,
    /// 敌方当前共享战技点。
    pub enemy_skill_points: u8,
    /// 按等待 AV 排序的存活单位普通行动位置。
    pub axis: Vec<AxisPosition>,
    /// 已经入场的单位，死亡记录保留。
    pub units: Vec<UnitResult>,
}

/// 一个暂停点的观测与合法选项，供自动策略、模型推理和训练读取。
///
/// `id`在单个会话内递增；提交时携带它，避免旧决策或重复提交影响新局面。
/// `None`表示放弃本次插入机会，仅在`Insert`决策点允许。
#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
pub struct Decision {
    /// 所属对象的稳定标识。
    pub id: u64,
    /// 当前选招窗口。
    pub point: DecisionPoint,
    /// 此暂停点的战况。
    pub observation: Observation,
    /// 此暂停点可提交的合法操作。
    pub legal: Vec<Operation>,
}
/// Return a member of legal, or None to decline inserts. A turn requires a choice.
pub trait Policy {
    /// 从 legal 中选择操作；插入窗口可返回 None 表示放弃。
    fn decide(
        &mut self,
        point: DecisionPoint,
        state: &Observation,
        legal: &[Operation],
    ) -> Option<Operation>;
}
/// 选择合法列表首项的默认策略。
pub struct Automatic;
impl Policy for Automatic {
    /// 从 legal 中选择操作；插入窗口可返回 None 表示放弃。
    fn decide(
        &mut self,
        _: DecisionPoint,
        _: &Observation,
        legal: &[Operation],
    ) -> Option<Operation> {
        legal.first().cloned()
    }
}
/// Script closure receives the same observation and legal list as Automatic.
impl<F> Policy for F
where
    F: FnMut(DecisionPoint, &Observation, &[Operation]) -> Option<Operation>,
{
    /// 从 legal 中选择操作；插入窗口可返回 None 表示放弃。
    fn decide(&mut self, p: DecisionPoint, s: &Observation, l: &[Operation]) -> Option<Operation> {
        self(p, s, l)
    }
}
