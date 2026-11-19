//! 局面运营层组装的单场配置与显式战斗规则。
use super::*;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 局面运营层组装完成的单场配置，调用方保证静态数据与引用完整。
///
/// 账号面板经局面运营和战斗准备后传入；额外局内乘区留待中间层接入。
pub struct Scenario {
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 本场选用的机制配置。
    pub mechanics: Mechanics,
    /// Sum of cycle allowances; current-cycle-capped revival charges consume it.
    /// Exhaustion yields
    /// [`Outcome::Defeat`] / [`Termination::AvBudgetExhausted`], unless terminal
    /// elimination has already settled. Equal-time order is an explicit mechanics policy.
    pub max_av: f64,
    /// Maximum executed actions, including ultimates, shared across waves.
    /// Reaching this safety limit yields [`Outcome::Incomplete`] /
    /// [`Termination::ActionLimit`] after terminal, elapsed-budget and cancellation checks.
    pub max_actions: u32,
    /// 我方共享战技点的初值和上限。
    pub ally_skill_points: SkillPoints,
    /// 敌方共享战技点的初值和上限。
    pub enemy_skill_points: SkillPoints,
    /// 我方参战单位。
    pub allies: Vec<Unit>,
    /// 敌方波次，队列模式只使用第一项。
    pub waves: Vec<Vec<Unit>>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 本场选择的时序、状态、元素和行动规则。
pub struct Mechanics {
    /// Some enables a finite queue in `waves[0]`; remaining waves must be absent.
    #[cfg_attr(
        feature = "diagnostics",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub queue_capacity: Option<usize>,
    /// Explicit immutable natural weaknesses. Missing units remain unknown.
    #[cfg_attr(
        feature = "diagnostics",
        serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")
    )]
    pub natural_weaknesses: std::collections::BTreeMap<String, Vec<crate::elements::Element>>,
    /// Weakness contributions owned by the corresponding active modifier instance.
    #[cfg_attr(
        feature = "diagnostics",
        serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")
    )]
    pub modifier_weaknesses: std::collections::BTreeMap<String, Vec<crate::elements::Element>>,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 按单位 ID 关联的七元素面板。
    pub element_profiles: std::collections::BTreeMap<String, crate::elements::ElementProfile>,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 本场共用的属性状态规则，按状态 ID 关联。
    pub modifier_definitions:
        std::collections::BTreeMap<String, crate::modifiers::ModifierDefinition>,
    /// Explicit initial actor variables; missing runtime reads are errors.
    #[cfg_attr(feature = "diagnostics", serde(default))]
    pub dynamic_values: std::collections::BTreeMap<String, std::collections::BTreeMap<String, f64>>,
    /// Empty uses one cycle of max_av. Otherwise their sum must equal max_av.
    #[cfg_attr(feature = "diagnostics", serde(default))]
    pub cycles: Vec<f64>,
    #[cfg_attr(feature = "diagnostics", serde(default = "one"))]
    /// 我方直接伤害的可选乘数，默认 1，取有限非负值。每次命中结算一次。
    pub ally_damage_factor: f64,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 按声明顺序执行的规则监听器。
    pub triggers: Vec<Trigger>,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 单位当前韧性或按单位关联的初始韧性。
    pub toughness: std::collections::BTreeMap<String, f64>,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 按单位 ID 关联的终结技优先级。
    pub ultimate_priority: std::collections::BTreeMap<String, i32>,
    /// Explicit tie policy pending game observation; never inferred from numeric values.
    #[cfg_attr(feature = "diagnostics", serde(default))]
    pub larger_priority_first: bool,
    #[cfg_attr(feature = "diagnostics", serde(default))]
    /// 是否先处理同刻的单位行动及即时插入。
    pub unit_before_cycle: bool,
}

#[cfg(feature = "diagnostics")]
/// 返回诊断文件中伤害系数的默认值一。
fn one() -> f64 {
    1.0
}

impl Default for Mechanics {
    /// 创建所属类型的默认配置或空运行集合。
    fn default() -> Self {
        Self {
            queue_capacity: None,
            natural_weaknesses: Default::default(),
            modifier_weaknesses: Default::default(),
            element_profiles: Default::default(),
            modifier_definitions: Default::default(),
            dynamic_values: Default::default(),
            cycles: vec![],
            ally_damage_factor: 1.0,
            triggers: vec![],
            toughness: Default::default(),
            ultimate_priority: Default::default(),
            larger_priority_first: false,
            unit_before_cycle: false,
        }
    }
}
