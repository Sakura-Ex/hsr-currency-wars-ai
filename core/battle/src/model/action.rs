//! 动作、效果树与条件的输入定义。
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 伤害、治疗和护盾取基础量时使用的属性。
pub enum Scale {
    /// 以攻击力作为基础量。
    Attack,
    /// 以防御力作为基础量。
    Defense,
    /// 以生命上限作为基础量。
    MaxHp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 效果取得存活目标的规则及绑定时机。
pub enum Target {
    /// One target explicitly bound by a Retarget effect; nested scopes restore it.
    ContextTarget,
    /// Select lowest HP fraction once at Action entry and retain it for every hit.
    AbilityTarget,
    /// 选择当前生命比例最低的存活敌人。
    EnemyLowestHp,
    /// 按嘲讽权重随机选择存活敌人。
    EnemyRandomTaunt,
    /// 等概率选择存活敌人。
    EnemyRandomUniform,
    /// 选择全部存活敌人；状态条件检查其中任意一个。
    AllEnemies,
    /// 选择生命比例最低的存活友方，包含自身。
    AllyLowestHp,
    /// 选择全部存活友方，包含自身。
    AllAllies,
    /// 选择当前效果执行者自身。
    SelfUnit,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(
    feature = "diagnostics",
    serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)
)]
/// 按声明顺序执行的效果树，分支只携带各自所需参数。
pub enum Effect {
    /// A fresh invocation frame, removed on return even after a runtime error.
    WithDynamicScope {
        /// 临时变量所属调用范围。
        scope: crate::dynamic_scope::DynamicScope,
        /// 依次执行的子效果。
        effects: Vec<Effect>,
    },
    /// 在执行者当前临时帧内写入变量。
    SetScopedDynamicValue {
        /// 临时变量所属调用范围。
        scope: crate::dynamic_scope::DynamicScope,
        /// 单位或临时作用域中的变量名。
        key: String,
        /// 要写入、计算或应用的数值。
        value: crate::expression::Expression,
    },
    /// One sourced hit with explicit element panels; no implicit toughness effect.
    ElementalDamage {
        /// 基础量采用的静态属性。
        scale: Scale,
        /// 基础属性的倍率，动态版本按执行者变量求值。
        ratio: crate::expression::Expression,
        /// 基础量的固定加值。
        flat: crate::expression::Expression,
        /// 本次伤害的固定或随机元素规则。
        element: crate::elements::DamageElement,
        /// 本次命中类别。
        attack_type: AttackType,
        /// 本次伤害是否抽取暴击。
        can_crit: bool,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// Source DamagePerformFinish boundary, distinct from completion of Action.
    AttackFinish,
    /// 在本动作已命中且仍存活的目标中随机绑定一个。
    RetargetAttackRandom {
        /// 依次执行的子效果。
        effects: Vec<Effect>,
        /// 没有合适的存活已命中目标时执行的效果。
        empty_effects: Vec<Effect>,
    },
    /// 按定义施加或刷新属性状态。
    AddModifier {
        /// 所属对象的稳定标识。
        id: String,
        /// 本次施加或当前持有的状态层数。
        stacks: crate::expression::Expression,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 按状态 ID 和来源范围移除状态。
    RemoveModifier {
        /// 所属对象的稳定标识。
        id: String,
        /// 为 true 时移除该 ID 的全部来源。
        from_any_source: bool,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 按当前条件选择一个效果分支。
    Conditional {
        /// 决定执行分支的条件。
        condition: Condition,
        /// 条件成立时依次执行的效果。
        then_effects: Vec<Effect>,
        /// 条件不成立时依次执行的效果。
        else_effects: Vec<Effect>,
    },
    /// 在循环入口求值次数，再依次执行循环体。
    Repeat {
        /// 循环入口求值一次的执行次数。
        count: crate::expression::Expression,
        /// 依次执行的子效果。
        effects: Vec<Effect>,
    },
    /// 按接受者的变量环境计算并写入持久变量。
    SetDynamicValue {
        /// 单位或临时作用域中的变量名。
        key: String,
        /// 要写入、计算或应用的数值。
        value: crate::expression::Expression,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 按执行者表达式计算能量变化。
    DynamicEnergy {
        /// 按所属效果解释的数值或变化量。
        amount: crate::expression::Expression,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 使用固定基础量和命中次数执行直接伤害。
    Damage {
        /// 基础量采用的静态属性。
        scale: Scale,
        /// 基础属性的倍率，动态版本按执行者变量求值。
        ratio: f64,
        /// 基础量的固定加值。
        flat: f64,
        /// 采用同一基础量的命中次数。
        hits: u8,
        /// 本次伤害是否抽取暴击。
        can_crit: bool,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 计算治疗并限制到接受者生命上限。
    Heal {
        /// 基础量采用的静态属性。
        scale: Scale,
        /// 基础属性的倍率，动态版本按执行者变量求值。
        ratio: f64,
        /// 基础量的固定加值。
        flat: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 计算护盾并按容量和持续时间叠加。
    Shield {
        /// 基础量采用的静态属性。
        scale: Scale,
        /// 基础属性的倍率，动态版本按执行者变量求值。
        ratio: f64,
        /// 基础量的固定加值。
        flat: f64,
        /// 本效果允许叠加到的护盾上限。
        max_total: f64,
        /// 护盾经历持有者普通回合开始的计时次数。
        duration_turns: u16,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 增减目标能量并限制到合法资源范围。
    Energy {
        /// 按所属效果解释的数值或变化量。
        amount: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 设置当前速度并保留行动距离。
    Speed {
        /// 要写入、计算或应用的数值。
        value: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 将距离提前量累计到下次普通回合重排。
    AdvanceNext {
        /// 以标准行动距离为基数的提前比例，负值表示延后。
        fraction: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 扣除韧性，降至零时触发击破事件。
    Toughness {
        /// 按所属效果解释的数值或变化量。
        amount: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 登记后续终结技、额外回合或追击。
    Request {
        /// 本对象的动作、请求或记录类别。
        kind: RequestKind,
        /// 可选动作标识或该执行者动作列表中的索引。
        action: Option<String>,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 立即调整当前剩余行动距离。
    Advance {
        /// 以标准行动距离为基数的提前比例，负值表示延后。
        fraction: f64,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 数值条件采用的比较运算。
pub enum Comparison {
    /// 两侧数值相等。
    Equal,
    /// 两侧数值不相等。
    NotEqual,
    /// 左侧大于右侧。
    Greater,
    /// 左侧大于或等于右侧。
    GreaterEqual,
    /// 左侧小于右侧。
    Less,
    /// 左侧小于或等于右侧。
    LessEqual,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(
    feature = "diagnostics",
    serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)
)]
/// 使用当前动作、命中上下文和变量判断的条件树。
pub enum Condition {
    /// Only self, bound ability target or any living enemy; never consumes RNG.
    HasModifier {
        /// 所属对象的稳定标识。
        id: String,
        /// 效果目标选择器或记录中的目标 ID。
        target: Target,
    },
    /// 检查当前事件所关联的命中类别。
    HitAttackType {
        /// 本次命中类别。
        attack_type: AttackType,
    },
    /// 对两侧表达式求值后按指定运算比较。
    Compare {
        /// 左侧表达式。
        left: crate::expression::Expression,
        /// 两侧数值使用的比较运算。
        comparison: Comparison,
        /// 右侧表达式。
        right: crate::expression::Expression,
    },
    /// 检查当前完整动作类别。
    ActionKind {
        /// 本对象的动作、请求或记录类别。
        kind: ActionKind,
    },
    /// 对单个条件取反。
    Not {
        /// 决定执行分支的条件。
        condition: Box<Condition>,
    },
    /// 所有子条件都成立，按顺序短路求值。
    All {
        /// 按顺序短路求值的条件列表。
        conditions: Vec<Condition>,
    },
    /// 任一子条件成立，按顺序短路求值。
    Any {
        /// 按顺序短路求值的条件列表。
        conditions: Vec<Condition>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 完整动作的技能类别，参与选招与请求合法性判断。
pub enum ActionKind {
    /// 普通回合或额外回合可选择的普攻。
    Basic,
    /// 普通回合或额外回合可选择的战技。
    Skill,
    /// 入插入 Turn 时先付费的终结技。
    Ultimate,
    /// 在完整动作边界调度、直接执行的追击。
    FollowUp,
}

/// Hit classification is independent of the enclosing skill/action kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
pub enum AttackType {
    /// 普通直接命中。
    Direct,
    /// 技能内部标记的追加命中。
    Pursued,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 一次技能的费用、有序效果和完成收益。
pub struct Action {
    /// 所属对象的稳定标识。
    pub id: String,
    /// 完整动作的技能类别。
    pub kind: ActionKind,
    /// 动作开始前扣除的战技点。
    pub sp_cost: u8,
    /// 动作完成且执行者存活时获得的战技点。
    pub sp_gain: u8,
    /// 动作开始前扣除的能量。
    pub energy_cost: f64,
    /// 动作完成且执行者存活时获得的能量。
    pub energy_gain: f64,
    /// 依次执行的子效果。
    pub effects: Vec<Effect>,
}
