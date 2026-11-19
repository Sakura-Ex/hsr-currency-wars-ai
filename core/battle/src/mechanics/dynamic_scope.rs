//! Explicit transient storage for sourced ability and task-template invocations.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "diagnostics", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "diagnostics", serde(rename_all = "snake_case"))]
/// 一次能力或任务模板调用的临时变量范围。
pub enum DynamicScope {
    /// 当前能力调用的临时变量范围。
    Ability,
    /// 当前任务模板调用的临时变量范围。
    TaskTemplate,
}

impl DynamicScope {
    /// 返回规则标识对应的稳定字符串，供诊断记录使用。
    #[cfg(feature = "diagnostics")]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ability => "ability",
            Self::TaskTemplate => "task_template",
        }
    }
}

/// 归属于特定单位与调用范围的临时变量表。
struct Frame {
    /// 本次调用所属的单位索引。
    owner: usize,
    /// 临时变量所属调用范围。
    scope: DynamicScope,
    /// 仅属于本次临时调用的变量表。
    values: BTreeMap<String, f64>,
}

#[derive(Default)]
/// 临时调用帧栈，最内层匹配帧遮蔽外层调用。
pub(crate) struct ScopeFrames(Vec<Frame>);

impl ScopeFrames {
    /// 为 owner 的本次 scope 调用创建空变量帧。
    pub fn push(&mut self, owner: usize, scope: DynamicScope) {
        self.0.push(Frame {
            owner,
            scope,
            values: BTreeMap::new(),
        });
    }
    /// 移除当前最内层调用帧。
    pub fn pop(&mut self) {
        self.0.pop();
    }
    /// 从最内层匹配 owner 与 scope 的帧中读取变量。
    pub fn get(&self, owner: usize, scope: DynamicScope, key: &str) -> Option<f64> {
        // A matching inner frame shadows the whole outer frame, even when the
        // requested key is absent. Never fall through to a previous invocation.
        self.0
            .iter()
            .rev()
            .find(|f| f.owner == owner && f.scope == scope)
            .and_then(|f| f.values.get(key).copied())
    }
    /// 写入最内层匹配帧，缺少活动调用时返回错误。
    pub fn set(
        &mut self,
        owner: usize,
        scope: DynamicScope,
        key: &str,
        value: f64,
    ) -> Result<(), String> {
        let frame = self
            .0
            .iter_mut()
            .rev()
            .find(|f| f.owner == owner && f.scope == scope)
            .ok_or_else(|| format!("no active {scope:?} dynamic scope for owner"))?;
        frame.values.insert(key.into(), value);
        Ok(())
    }
}
