//! 战斗监听事件、插入请求与触发器声明。
use super::*;

/// Runtime events are dispatched even when replay recording is disabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
pub enum BattleEvent {
    /// 建立本场状态后分派的全局开战事件。
    BattleStart,
    /// 每次装载新波次后分派的全局事件。
    WaveStart,
    /// 单位回合开始，选招或执行技能前分派。
    TurnStart,
    /// 单位回合收尾时分派。
    TurnEnd,
    /// 完整动作付费后开始执行时分派。
    ActionStart,
    /// 开始执行技能效果列表前分派。
    BeforeSkillUse,
    /// 由显式 AttackFinish 效果分派的攻击段结束。
    AfterAttack,
    /// 完整动作效果及收益结算后分派。
    ActionEnd,
    /// 本击扣盾扣血后分派。
    Hit,
    /// 韧性从正值扣至零时分派。
    Break,
    /// 本次命中造成死亡后分派。
    Death,
    /// 当前倒计时轮次收尾时分派的全局事件。
    CycleEnd,
}

impl BattleEvent {
    /// Global events have no unit subject and tick every present owner's lifetime.
    pub fn is_global(self) -> bool {
        matches!(self, Self::BattleStart | Self::WaveStart | Self::CycleEnd)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 后续动作采用的付费、选招及回合调度方式。
pub enum RequestKind {
    /// 入插入 Turn 时先付费的终结技。
    Ultimate,
    /// 进入额外回合并在回合开始事件后选招。
    ExtraTurn,
    /// 在完整动作边界调度、直接执行的追击。
    FollowUp,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 指定单位在匹配事件上执行效果的监听规则。
pub struct Trigger {
    /// 监听器所属的单位实例 ID。
    pub owner: String,
    /// 监听或寿命计时匹配的规则事件。
    pub event: BattleEvent,
    /// Unit-event subject filter; global events require None.
    pub subject: Option<String>,
    /// 可选动作标识或该执行者动作列表中的索引。
    pub action: Option<String>,
    /// Zero means unlimited; positive counts are consumed before dispatch.
    pub uses: u32,
    /// 依次执行的子效果。
    pub effects: Vec<Effect>,
    /// Expire before any listeners of this event. Global ticks apply to present
    /// owners; unit ticks apply only when the owner is the event subject.
    pub expires_on: Option<BattleEvent>,
    /// 剩余或初始寿命事件计数。
    pub duration: u32,
}
