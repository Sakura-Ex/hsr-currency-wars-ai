//! Runtime numeric expressions. Variable lookup is supplied by the owning state.
use crate::dynamic_scope::DynamicScope;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(
    feature = "diagnostics",
    serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)
)]
/// 由常量、变量和算术运算构成的动态数值树。
pub enum Expression {
    /// 直接返回有限常量。
    Constant {
        /// 要写入、计算或应用的数值。
        value: f64,
    },
    /// 读取单位持久变量。
    Variable {
        /// 单位或临时作用域中的变量名。
        key: String,
    },
    /// 读取最内层匹配调用帧中的临时变量。
    ScopedVariable {
        /// 临时变量所属调用范围。
        scope: DynamicScope,
        /// 单位或临时作用域中的变量名。
        key: String,
    },
    /// 计算两侧之和。
    Add {
        /// 左侧表达式。
        left: Box<Self>,
        /// 右侧表达式。
        right: Box<Self>,
    },
    /// 计算左侧减右侧。
    Subtract {
        /// 左侧表达式。
        left: Box<Self>,
        /// 右侧表达式。
        right: Box<Self>,
    },
    /// 计算两侧乘积。
    Multiply {
        /// 左侧表达式。
        left: Box<Self>,
        /// 右侧表达式。
        right: Box<Self>,
    },
    /// 计算左侧除以右侧，分母为零时返回错误。
    Divide {
        /// 左侧表达式。
        left: Box<Self>,
        /// 右侧表达式。
        right: Box<Self>,
    },
    /// 取两侧数值的较大值。
    Maximum {
        /// 左侧表达式。
        left: Box<Self>,
        /// 右侧表达式。
        right: Box<Self>,
    },
    /// 向下取整。
    Floor {
        /// 要写入、计算或应用的数值。
        value: Box<Self>,
    },
    /// 对数值取负。
    Negate {
        /// 要写入、计算或应用的数值。
        value: Box<Self>,
    },
}

impl Expression {
    /// Missing bindings and invalid arithmetic fail; neither becomes a zero value.
    pub fn evaluate(&self, lookup: &impl Fn(&str) -> Option<f64>) -> Result<f64, String> {
        self.evaluate_with_scopes(lookup, &|_, _| None)
    }
    /// Transient values require an explicit active frame; no unit fallback.
    pub fn evaluate_with_scopes(
        &self,
        lookup: &impl Fn(&str) -> Option<f64>,
        scoped: &impl Fn(DynamicScope, &str) -> Option<f64>,
    ) -> Result<f64, String> {
        let eval = |child: &Self| child.evaluate_with_scopes(lookup, scoped);
        let value = match self {
            Self::Constant { value } => *value,
            Self::Variable { key } => {
                lookup(key).ok_or_else(|| format!("unresolved dynamic key {key}"))?
            }
            Self::ScopedVariable { scope, key } => scoped(*scope, key)
                .ok_or_else(|| format!("unresolved {scope:?} dynamic key {key}"))?,
            Self::Add { left, right } => eval(left)? + eval(right)?,
            Self::Subtract { left, right } => eval(left)? - eval(right)?,
            Self::Multiply { left, right } => eval(left)? * eval(right)?,
            Self::Divide { left, right } => {
                let denominator = eval(right)?;
                if denominator == 0.0 {
                    return Err("dynamic division by zero".into());
                }
                eval(left)? / denominator
            }
            Self::Negate { value } => -eval(value)?,
            Self::Floor { value } => eval(value)?.floor(),
            Self::Maximum { left, right } => eval(left)?.max(eval(right)?),
        };
        if !value.is_finite() {
            return Err("nonfinite dynamic expression result".into());
        }
        Ok(value)
    }
}
