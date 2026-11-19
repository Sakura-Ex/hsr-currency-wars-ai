//! Explicit seven-element panels for attacks that select their damage element.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 伤害、元素面板和弱点共用的七元素标识。
pub enum Element {
    /// 物理元素。
    Physical,
    /// 火元素。
    Fire,
    /// 冰元素。
    Ice,
    /// 雷元素。
    Thunder,
    /// 风元素。
    Wind,
    /// 量子元素。
    Quantum,
    /// 虚数元素。
    Imaginary,
}

impl Element {
    /// 七种元素的固定遍历和随机抽样顺序。
    pub const ALL: [Self; 7] = [
        Self::Physical,
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Wind,
        Self::Quantum,
        Self::Imaginary,
    ];
    /// 返回规则标识对应的稳定字符串，供诊断记录使用。
    #[cfg(feature = "diagnostics")]
    pub fn name(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Fire => "fire",
            Self::Ice => "ice",
            Self::Thunder => "thunder",
            Self::Wind => "wind",
            Self::Quantum => "quantum",
            Self::Imaginary => "imaginary",
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(
    feature = "diagnostics",
    serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)
)]
/// 一次显式元素命中的元素选择方式。
pub enum DamageElement {
    /// 使用明确指定的固定元素。
    Fixed {
        /// 本次伤害的固定或随机元素规则。
        element: Element,
    },
    /// 七种元素等概率随机选择。
    RandomAll,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(deny_unknown_fields))]
/// 完整七元素增伤与抗性，静态资料由组装方提供。
pub struct ElementProfile {
    /// Additional element-specific bonus; excludes Stats.damage_bonus and modifiers.
    pub damage_bonus: BTreeMap<Element, f64>,
    /// Complete resistance by element, replacing the legacy resolved scalar.
    pub resistance: BTreeMap<Element, f64>,
}
