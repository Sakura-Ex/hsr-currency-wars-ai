//! Explicit property-modifier storage. No raw Config defaults are inferred.
use crate::{expression::Expression, model::BattleEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 当前状态系统支持贡献的属性。
pub enum Property {
    /// 对暴击伤害加成作加法贡献。
    CritDamage,
    /// 对伤害提高池作加法贡献。
    DamageBonus,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 按表达式计算的一项每层属性贡献。
pub struct PropertyChange {
    /// 当前贡献修改的属性。
    pub property: Property,
    /// 施加或刷新状态时求值的每层贡献表达式。
    pub value: Expression,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 同名状态按来源叠加或替换的方式。
pub enum StackMode {
    /// 同 ID 同来源替换，不同来源共存。
    ReplaceBySource,
    /// 同 ID 同来源叠层并刷新，不同来源共存。
    AddBySource,
    /// 同 ID 跨来源替换。
    ReplaceGlobal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 有限寿命匹配事件主体的归属对象。
pub enum TickUnit {
    /// 跟随状态持有者的事件计时。
    Owner,
    /// 跟随状态施加者的事件计时。
    Source,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 状态按指定事件和单位扣减寿命的规则。
pub struct Lifetime {
    /// 监听或寿命计时匹配的规则事件。
    pub event: BattleEvent,
    /// 寿命跟随持有者或来源单位。
    pub unit: TickUnit,
    /// 初始寿命事件次数。
    pub ticks: u32,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 属性状态的叠层、寿命和每层贡献定义。
pub struct ModifierDefinition {
    /// 同 ID 状态的来源匹配及叠层方式。
    pub stack_mode: StackMode,
    /// 每个状态实例的层数上限。
    pub max_stacks: u32,
    /// 可选的有限寿命配置。
    pub lifetime: Option<Lifetime>,
    /// 每层属性贡献表达式。
    pub properties: Vec<PropertyChange>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Serialize))]
/// 活动属性状态的来源、层数与剩余寿命。
pub struct ModifierState {
    /// 所属对象的稳定标识。
    pub id: String,
    /// 施加该状态的单位 ID。
    pub source: String,
    /// 本次施加或当前持有的状态层数。
    pub stacks: u32,
    /// 剩余寿命事件次数，None 表示无有限计时。
    pub remaining_ticks: Option<u32>,
}

/// 状态定义与施加或刷新时求值的属性快照。
pub(crate) struct ModifierInstance {
    /// 所属对象的当前运行状态。
    pub state: ModifierState,
    /// 施加时使用的状态规则副本。
    pub definition: ModifierDefinition,
    // Values are evaluated at application/stack time and replaced on refresh.
    /// 施加或刷新时求值的每层属性贡献快照。
    pub values: Vec<(Property, f64)>,
}

#[derive(Default)]
/// 单个单位拥有的全部活动属性状态。
pub(crate) struct ModifierBook {
    /// 当前所有活动状态实例。
    pub instances: Vec<ModifierInstance>,
}

impl ModifierBook {
    /// 按 ID、来源和叠层规则施加状态，返回最终层数。
    pub fn apply(
        &mut self,
        id: &str,
        source: &str,
        stacks: u32,
        definition: &ModifierDefinition,
        values: Vec<(Property, f64)>,
    ) -> u32 {
        let position = self.instances.iter().position(|m| {
            m.state.id == id
                && (definition.stack_mode == StackMode::ReplaceGlobal || m.state.source == source)
        });
        let stacks = position
            .filter(|_| definition.stack_mode == StackMode::AddBySource)
            .map_or(stacks, |i| {
                self.instances[i].state.stacks.saturating_add(stacks)
            })
            .min(definition.max_stacks);
        let instance = ModifierInstance {
            state: ModifierState {
                id: id.into(),
                source: source.into(),
                stacks,
                remaining_ticks: definition.lifetime.as_ref().map(|l| l.ticks),
            },
            definition: definition.clone(),
            values,
        };
        if let Some(i) = position {
            self.instances[i] = instance;
        } else {
            self.instances.push(instance);
        }
        stacks
    }

    /// 按 ID 和可选来源移除状态，诊断构建返回被移除快照。
    pub fn remove(&mut self, id: &str, source: Option<&str>) -> Vec<ModifierState> {
        #[cfg(feature = "diagnostics")]
        let mut removed = vec![];
        #[cfg(not(feature = "diagnostics"))]
        let removed = vec![];
        self.instances.retain(|m| {
            let matches = m.state.id == id && source.is_none_or(|s| m.state.source == s);
            #[cfg(feature = "diagnostics")]
            if matches {
                removed.push(m.state.clone());
            }
            !matches
        });
        removed
    }

    /// 按事件和主体扣减寿命，移除到期状态并返回诊断快照。
    pub fn tick(
        &mut self,
        event: BattleEvent,
        subject: Option<&str>,
        owner: &str,
    ) -> Vec<ModifierState> {
        #[cfg(feature = "diagnostics")]
        let mut removed = vec![];
        #[cfg(not(feature = "diagnostics"))]
        let removed = vec![];
        self.instances.retain_mut(|m| {
            if let Some(lifetime) = &m.definition.lifetime {
                let unit = match lifetime.unit {
                    TickUnit::Owner => owner,
                    TickUnit::Source => &m.state.source,
                };
                if lifetime.event == event && (event.is_global() || subject == Some(unit)) {
                    let left = m
                        .state
                        .remaining_ticks
                        .as_mut()
                        .expect("有限寿命在状态施加时初始化");
                    *left -= 1;
                    if *left == 0 {
                        #[cfg(feature = "diagnostics")]
                        removed.push(m.state.clone());
                        return false;
                    }
                }
            }
            true
        });
        removed
    }

    /// 合计指定属性的每层贡献乘层数，非有限合计返回错误。
    pub fn delta(&self, property: Property) -> Result<f64, String> {
        let value: f64 = self
            .instances
            .iter()
            .flat_map(|m| {
                m.values.iter().filter_map(move |(p, v)| {
                    (*p == property).then_some(v * f64::from(m.state.stacks))
                })
            })
            .sum();
        if !value.is_finite() {
            return Err("nonfinite modifier property sum".into());
        }
        Ok(value)
    }

    /// 导出当前活动状态的来源、层数和余寿命。
    pub fn states(&self) -> Vec<ModifierState> {
        self.instances.iter().map(|m| m.state.clone()).collect()
    }
}
