//! 单位的静态面板、初始资源与复活配置。
use super::*;

/// Effective values, already composed by the caller. No account data is guessed.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
pub struct Stats {
    /// 单位等级，参与攻击方的防御乘区。
    pub level: u16,
    /// 生命上限，作为生命倍率、治疗封顶与复活的基数。
    pub max_hp: f64,
    /// 静态攻击力。
    pub attack: f64,
    /// 静态防御力或本击目标防御。
    pub defense: f64,
    /// 当前或入场速度，等待 AV 等于剩余距离除以速度。
    pub speed: f64,
    /// 加权随机选敌使用的正嘲讽权重。
    pub taunt: f64,
    /// 逐命中暴击概率，比例以小数表示。
    pub crit_rate: f64,
    /// 暴击伤害加成，暴击倍率为一加此值。
    pub crit_damage: f64,
    /// 加法增伤池的比例贡献。
    pub damage_bonus: f64,
    /// 攻击方携带的有效减防比例。
    pub defense_reduction: f64,
    /// 攻击方无视防御的比例。
    pub defense_ignore: f64,
    /// 目标抗性或按元素关联的完整抗性。
    pub resistance: f64,
    /// 攻击方的抗性穿透。
    pub resistance_penetration: f64,
    /// 目标易伤比例。
    pub vulnerability: f64,
    /// 目标减伤比例，保留倍率为一减此值。
    pub damage_reduction: f64,
    /// 已解析的独立倍率，中性值为一。
    pub independent_multiplier: f64,
    /// 目标是否处于韧性击破状态。
    pub toughness_broken: bool,
    /// 施放者的治疗加成。
    pub healing_bonus: f64,
    /// 接受者的受治疗加成。
    pub incoming_healing_bonus: f64,
    /// 施放者的护盾加成。
    pub shield_bonus: f64,
}

/// Optional explicit model of immediate revival. It is not inferred from game data.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
pub struct Revival {
    /// 初始可用复活次数。
    pub charges: u8,
    /// 每次复活恢复的生命上限比例。
    pub hp_fraction: f64,
    /// 复活时请求扣除的当前 Cycle 额度。
    pub av_penalty: f64,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 本场参战单位的静态定义与入场资源。
pub struct Unit {
    /// 所属对象的稳定标识。
    pub id: String,
    /// 组装完成的静态面板。
    pub stats: Stats,
    /// 入场生命值。
    pub initial_hp: f64,
    /// 入场能量。
    pub initial_energy: f64,
    /// 能量上限。
    pub max_energy: f64,
    /// 可选的即时复活规则。
    pub revival: Option<Revival>,
    /// First affordable action wins, with ultimates considered at action boundaries.
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 一方共享的战技点初值与上限。
pub struct SkillPoints {
    /// 一方初始共享战技点。
    pub initial: u8,
    /// 一方战技点上限。
    pub maximum: u8,
}
